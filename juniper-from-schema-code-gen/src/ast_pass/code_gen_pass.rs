mod gen_query_trails;

use super::{
    error::{Error, ErrorKind},
    ident, quote_ident, type_name, AstData, TypeKind,
};
use crate::{
    ast_pass::schema_visitor::SchemaVisitor,
    from_directive::{Deprecation, FieldArguments, FromDirective, JuniperDirective, Ownership},
    nullable_type::NullableType,
};
use graphql_parser::{
    query::{Name, Type},
    schema::{self, *},
    Pos,
};
use heck::{CamelCase, SnakeCase};
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    iter::Extend,
    string::ToString,
};
use syn::Ident;

type Result<T, E = ()> = std::result::Result<T, E>;

pub struct CodeGenPass<'doc> {
    tokens: TokenStream,
    error_type: syn::Type,
    context_type: syn::Type,
    errors: BTreeSet<Error<'doc>>,
    ast_data: AstData<'doc>,
    raw_schema: &'doc str,
}

impl<'doc> SchemaVisitor<'doc> for CodeGenPass<'doc> {
    fn visit_schema_definition(&mut self, schema_def: &'doc SchemaDefinition) {
        if schema_def.subscription.is_some() {
            self.emit_non_fatal_error(schema_def.position, ErrorKind::SubscriptionsNotSupported);
        }

        self.parse_directives(schema_def);

        let query = match &schema_def.query {
            Some(query) => ident(query),
            None => {
                self.emit_non_fatal_error(schema_def.position, ErrorKind::NoQueryType);
                return;
            }
        };

        let mutation = match &schema_def.mutation {
            Some(mutation) => quote_ident(mutation),
            None => {
                let context_type = &self.context_type;
                quote! { juniper::EmptyMutation<#context_type> }
            }
        };

        self.extend(quote! {
            /// The GraphQL schema type generated by `juniper-from-schema`.
            pub type Schema = juniper::RootNode<'static, #query, #mutation>;
        });
    }

    fn visit_scalar_type(&mut self, scalar_type: &'doc ScalarType) {
        self.parse_directives(scalar_type);

        match &*scalar_type.name {
            "Date" | "DateTime" | "Uuid" | "Url" => {
                if scalar_type.description.is_some() {
                    self.emit_non_fatal_error(
                        scalar_type.position,
                        ErrorKind::SpecialCaseScalarWithDescription,
                    );
                }
            }
            name => {
                let name = ident(name);
                let description = &scalar_type
                    .description
                    .as_ref()
                    .map(|desc| quote! { description: #desc })
                    .unwrap_or(quote! {});

                self.gen_scalar_type_with_data(&name, &description);
            }
        };
    }

    fn visit_object_type(&mut self, obj_type: &'doc ObjectType) {
        self.parse_directives(obj_type);

        let struct_name = ident(&obj_type.name);

        let trait_name = trait_map_for_struct_name(&struct_name);

        let field_tokens = obj_type
            .fields
            .iter()
            .map(|field| self.collect_data_for_field_gen(field))
            .collect::<Vec<_>>();

        let trait_methods = field_tokens
            .iter()
            .map(|field| {
                let field_name = &field.field_method;
                let field_type = &field.field_type;

                let args = &field.trait_args;

                let error_type = &self.error_type;
                let context_type = &self.context_type;

                match field.type_kind {
                    TypeKind::Scalar => {
                        quote! {
                            /// Field method generated by `juniper-from-schema`.
                            fn #field_name<'a>(
                                &self,
                                executor: &juniper::Executor<'a, #context_type>,
                                #(#args),*
                            ) -> std::result::Result<#field_type, #error_type>;
                        }
                    }
                    TypeKind::Type => {
                        let query_trail_type = ident(&field.inner_type);
                        let trail = quote! {
                            &QueryTrail<'a, #query_trail_type, juniper_from_schema::Walked>
                        };
                        quote! {
                            /// Field method generated by `juniper-from-schema`.
                            fn #field_name<'a>(
                                &self,
                                executor: &juniper::Executor<'a, #context_type>,
                                trail: #trail, #(#args),*
                            ) -> std::result::Result<#field_type, #error_type>;
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        self.extend(quote! {
            /// Trait for GraphQL field methods generated by `juniper-from-schema`.
            pub trait #trait_name {
                #(#trait_methods)*
            }
        });

        let fields = field_tokens
            .iter()
            .map(|field| gen_field(field, &struct_name, &trait_name, &self.error_type))
            .collect::<Vec<_>>();

        let description = obj_type
            .description
            .as_ref()
            .map(|d| quote! { description: #d })
            .unwrap_or_else(empty_token_stream);

        let interfaces = if obj_type.implements_interfaces.is_empty() {
            empty_token_stream()
        } else {
            let interface_names = obj_type.implements_interfaces.iter().map(|name| {
                let name = ident(name);
                quote! { &#name }
            });
            quote! { interfaces: [#(#interface_names),*] }
        };

        let context_type = &self.context_type;

        let code = quote! {
            juniper::graphql_object!(#struct_name: #context_type |&self| {
                #description
                #(#fields)*
                #interfaces
            });
        };
        self.extend(code)
    }

    fn visit_interface_type(&mut self, interface: &'doc InterfaceType) {
        self.parse_directives(interface);

        let interface_name = ident(&interface.name);

        let description = &interface
            .description
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(String::new);

        let implementors = self.get_implementors_of_interface(&interface.name);

        let implementors = if let Some(implementors) = implementors {
            implementors
        } else {
            return;
        };

        let implementors = implementors.iter().map(ident).collect::<Vec<_>>();

        // Enum
        let variants = implementors.iter().map(|name| {
            quote! { #name(#name) }
        });
        self.extend(quote! {
            pub enum #interface_name {
                #(#variants),*
            }
        });

        // From implementations
        for variant in &implementors {
            self.extend(quote! {
                impl std::convert::From<#variant> for #interface_name {
                    fn from(x: #variant) -> #interface_name {
                        #interface_name::#variant(x)
                    }
                }
            });
        }

        // Resolvers
        let instance_resolvers = implementors.iter().map(|name| {
            quote! {
                &#name => match *self { #interface_name::#name(ref h) => Some(h), _ => None }
            }
        });

        let field_tokens: Vec<FieldTokens> = interface
            .fields
            .iter()
            .map(|field| self.collect_data_for_field_gen(field))
            .collect::<Vec<_>>();

        let field_token_streams = field_tokens
            .iter()
            .map(|field| {
                let field_name = &field.name;
                let args = &field.macro_args;
                let field_type = &field.field_type;

                let description = doc_tokens(&field.description);

                let arms = implementors.iter().map(|variant| {
                    let trait_name = trait_map_for_struct_name(&variant);
                    let struct_name = variant;

                    let body = gen_field_body(&field, &quote! {inner}, &struct_name, &trait_name);

                    quote! {
                        #interface_name::#struct_name(ref inner) => {
                            #body
                        }
                    }
                });

                let all_args = to_field_args_list(&args);

                let error_type = &self.error_type;

                let deprecation = &field.deprecation;

                quote! {
                    #description
                    #deprecation
                    field #field_name(#all_args) -> std::result::Result<#field_type, #error_type> {
                        match *self {
                            #(#arms),*
                        }
                    }
                }
            })
            .collect::<Vec<_>>();

        let context_type = &self.context_type;

        let code = quote! {
            juniper::graphql_interface!(#interface_name: #context_type |&self| {
                description: #description

                #(#field_token_streams)*

                instance_resolvers: |_| {
                    #(#instance_resolvers),*
                }
            });
        };
        self.extend(code);
    }

    fn visit_union_type(&mut self, union: &'doc UnionType) {
        self.parse_directives(union);

        let union_name = ident(&union.name);
        let implementors = union.types.iter().map(ident).collect::<Vec<_>>();

        // Enum
        let variants = implementors.iter().map(|name| {
            quote! { #name(#name) }
        });
        self.extend(quote! {
            pub enum #union_name {
                #(#variants),*
            }
        });

        // From implementations
        for variant in &implementors {
            self.extend(quote! {
                impl std::convert::From<#variant> for #union_name {
                    fn from(x: #variant) -> #union_name {
                        #union_name::#variant(x)
                    }
                }
            })
        }

        // Resolvers
        let instance_resolvers = implementors.iter().map(|name| {
            quote! {
                &#name => match *self { #union_name::#name(ref h) => Some(h), _ => None }
            }
        });

        let description = union
            .description
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(String::new);

        let context_type = &self.context_type;

        let code = quote! {
            juniper::graphql_union!(#union_name: #context_type |&self| {
                description: #description

                instance_resolvers: |_| {
                    #(#instance_resolvers),*
                }
            });
        };
        self.extend(code);
    }

    fn visit_enum_type(&mut self, enum_type: &'doc EnumType) {
        self.parse_directives(enum_type);

        let name = to_enum_name(&enum_type.name);

        let values = enum_type
            .values
            .iter()
            .map(|enum_value| self.gen_enum_value(enum_value))
            .collect::<Vec<_>>();

        let description = doc_tokens(&enum_type.description);

        self.extend(quote! {
            #description
            #[derive(juniper::GraphQLEnum, Debug, Eq, PartialEq, Copy, Clone, Hash)]
            pub enum #name {
                #(#values)*
            }
        });
    }

    fn visit_input_object_type(&mut self, input_object: &'doc InputObjectType) {
        self.parse_directives(input_object);

        let name = ident(&input_object.name);

        let fields = input_object
            .fields
            .iter()
            .map(|field| {
                if field.default_value.is_some() {
                    self.emit_non_fatal_error(
                        field.position,
                        ErrorKind::InputTypeFieldWithDefaultValue,
                    );
                }

                let arg = self.argument_to_name_and_rust_type(&field);
                let name = ident(arg.name);
                let rust_type = arg.macro_type;

                let description = doc_tokens(&field.description);

                quote! {
                    #[allow(missing_docs)]
                    #description
                    #name: #rust_type
                }
            })
            .collect::<Vec<_>>();

        let description = doc_tokens(&input_object.description);

        self.extend(quote! {
            #[derive(juniper::GraphQLInputObject, Debug)]
            #description
            pub struct #name {
                #(#fields),*
            }
        })
    }

    fn visit_directive_definition(&mut self, inner: &'doc schema::DirectiveDefinition) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }

    fn visit_scalar_type_extension(&mut self, inner: &'doc schema::ScalarTypeExtension) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }

    fn visit_object_type_extension(&mut self, inner: &'doc schema::ObjectTypeExtension) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }

    fn visit_interface_type_extension(&mut self, inner: &'doc schema::InterfaceTypeExtension) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }

    fn visit_union_type_extension(&mut self, inner: &'doc schema::UnionTypeExtension) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }

    fn visit_enum_type_extension(&mut self, inner: &'doc schema::EnumTypeExtension) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }

    fn visit_input_object_type_extension(&mut self, inner: &'doc schema::InputObjectTypeExtension) {
        self.emit_non_fatal_error(inner.position, ErrorKind::TypeExtensionNotSupported)
    }
}

impl<'doc> CodeGenPass<'doc> {
    pub fn new(
        raw_schema: &'doc str,
        error_type: syn::Type,
        context_type: syn::Type,
        ast_data: AstData<'doc>,
    ) -> Self {
        CodeGenPass {
            tokens: quote! {},
            error_type,
            context_type,
            ast_data,
            errors: BTreeSet::new(),
            raw_schema,
        }
    }

    pub fn gen_juniper_code(
        mut self,
        doc: &'doc Document,
    ) -> std::result::Result<TokenStream, BTreeSet<Error<'doc>>> {
        self.validate_doc(doc);
        self.check_for_errors()?;

        self.gen_query_trails(doc);
        self.visit_document(doc);

        self.check_for_errors()?;
        Ok(self.tokens)
    }

    fn validate_doc(&mut self, doc: &'doc Document) {
        let mut case_validator = FieldNameCaseValidator::new(self);
        case_validator.visit_document(doc);
    }

    fn check_for_errors(&self) -> Result<(), BTreeSet<Error<'doc>>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    pub fn is_date_time_scalar_defined(&self) -> bool {
        self.ast_data.date_time_scalar_defined()
    }

    pub fn is_date_scalar_defined(&self) -> bool {
        self.ast_data.date_scalar_defined()
    }

    pub fn is_uuid_scalar_defined(&self) -> bool {
        self.ast_data.uuid_scalar_defined()
    }

    pub fn is_url_scalar_defined(&self) -> bool {
        self.ast_data.url_scalar_defined()
    }

    pub fn is_scalar(&self, name: &str) -> bool {
        self.ast_data.is_scalar(name)
    }

    pub fn is_enum(&self, name: &str) -> bool {
        self.ast_data.is_enum_variant(name)
    }

    pub fn get_implementors_of_interface(&self, name: &str) -> Option<&Vec<&str>> {
        self.ast_data.get_interface_implementor(name)
    }

    fn emit_non_fatal_error(&mut self, pos: Pos, kind: ErrorKind<'doc>) {
        let error = Error {
            pos,
            kind,
            raw_schema: &self.raw_schema,
        };
        self.errors.insert(error);
    }

    fn emit_fatal_error(&mut self, pos: Pos, kind: ErrorKind<'doc>) -> Result<()> {
        self.emit_non_fatal_error(pos, kind);
        Err(())
    }

    fn gen_scalar_type_with_data(&mut self, name: &Ident, description: &TokenStream) {
        // We explicity don't implement `From<T> where T: Into<String>` because then users wouldn't
        // be able to add their own `From` implementations, since `From<T>` overlaps with other
        // implementations.
        self.extend(quote! {
            /// Custom scalar type generated by `juniper-from-schema`.
            #[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Clone, Hash)]
            pub struct #name(pub String);

            juniper::graphql_scalar!(#name {
                #description

                resolve(&self) -> juniper::Value {
                    juniper::Value::string(&self.0)
                }

                from_input_value(v: &InputValue) -> Option<#name> {
                    v.as_string_value().map(|s| #name::new(s.to_owned()))
                }

                from_str<'a>(value: ScalarToken<'a>) -> juniper::ParseScalarResult<'a> {
                    <String as juniper::ParseScalarValue>::from_str(value)
                }
            });

            impl #name {
                fn new<T: Into<String>>(t: T) -> Self {
                    #name(t.into())
                }
            }

            impl std::ops::Deref for #name {
                type Target = str;

                fn deref(&self) -> &Self::Target {
                    &self.0
                }
            }

            impl std::ops::DerefMut for #name {
                fn deref_mut(&mut self) -> &mut Self::Target {
                    &mut self.0
                }
            }
        })
    }

    fn argument_to_name_and_rust_type(&mut self, arg: &'doc InputValue) -> FieldArgument<'doc> {
        self.parse_directives(arg);

        let default_value_tokens = arg
            .default_value
            .as_ref()
            .map(|value| self.quote_value(&value, type_name(&arg.value_type), arg.position));

        let arg_name = arg.name.to_snake_case();

        let (macro_type, _) = self.gen_field_type(
            &arg.value_type,
            &FieldTypeDestination::Argument,
            false,
            arg.position,
        );

        let (trait_type, _) = self.gen_field_type(
            &arg.value_type,
            &FieldTypeDestination::Argument,
            default_value_tokens.is_some(),
            arg.position,
        );

        FieldArgument {
            name: arg_name,
            macro_type,
            trait_type,
            default_value: default_value_tokens,
            description: &arg.description,
        }
    }

    fn gen_field_type(
        &mut self,
        field_type: &Type,
        destination: &FieldTypeDestination,
        has_default_value: bool,
        pos: Pos,
    ) -> (TokenStream, TypeKind) {
        let field_type = NullableType::from_schema_type(field_type);

        if has_default_value && !field_type.is_nullable() {
            self.emit_non_fatal_error(pos, ErrorKind::NonnullableFieldWithDefaultValue);
        }

        let field_type = if has_default_value {
            field_type.remove_one_layer_of_nullability()
        } else {
            field_type
        };

        let as_ref = match destination {
            FieldTypeDestination::Return(attrs) => match attrs.ownership {
                Ownership::AsRef => true,
                Ownership::Borrowed => false,
                Ownership::Owned => false,
            },
            FieldTypeDestination::Argument => false,
        };

        let (tokens, ty) = self.gen_nullable_field_type(field_type, as_ref, pos);

        match (destination, ty) {
            (FieldTypeDestination::Return(attrs), ref ty) => match attrs.ownership {
                Ownership::Owned | Ownership::AsRef => (tokens, *ty),
                Ownership::Borrowed => (quote! { &#tokens }, *ty),
            },

            (FieldTypeDestination::Argument, ty @ TypeKind::Scalar) => (tokens, ty),
            (FieldTypeDestination::Argument, ty @ TypeKind::Type) => (tokens, ty),
        }
    }

    fn gen_nullable_field_type(
        &mut self,
        field_type: NullableType,
        as_ref: bool,
        pos: Pos,
    ) -> (TokenStream, TypeKind) {
        use crate::nullable_type::NullableType::*;

        match field_type {
            NamedType(name) => {
                if as_ref {
                    self.emit_non_fatal_error(pos, ErrorKind::AsRefOwnershipForNamedType);
                }

                self.graphql_scalar_type_to_rust_type(&name, pos)
            }
            ListType(item_type) => {
                let (item_type, ty) = self.gen_nullable_field_type(*item_type, false, pos);
                let tokens = if as_ref {
                    quote! { Vec<&#item_type> }
                } else {
                    quote! { Vec<#item_type> }
                };
                (tokens, ty)
            }
            NullableType(item_type) => {
                let (item_type, ty) = self.gen_nullable_field_type(*item_type, false, pos);
                let tokens = if as_ref {
                    quote! { Option<&#item_type> }
                } else {
                    quote! { Option<#item_type> }
                };
                (tokens, ty)
            }
        }
    }

    fn collect_data_for_field_gen(&mut self, field: &'doc Field) -> FieldTokens<'doc> {
        let name = ident(&field.name);

        let inner_type = type_name(&field.field_type).to_camel_case();

        let attributes = self.parse_directives(field);
        let deprecation = attributes
            .deprecated
            .as_ref()
            .map(quote_deprecation)
            .unwrap_or_else(empty_token_stream);

        let (field_type, type_kind) = self.gen_field_type(
            &field.field_type,
            &FieldTypeDestination::Return(attributes),
            false,
            field.position,
        );

        let field_method = ident(format!("field_{}", name.to_string().to_snake_case()));

        let args_data = field
            .arguments
            .iter()
            .map(|input_value| self.argument_to_name_and_rust_type(&input_value))
            .collect::<Vec<_>>();

        let macro_args = args_data
            .iter()
            .map(|arg| {
                let name = ident(&arg.name);
                let arg_type = &arg.macro_type;
                let description = doc_tokens(&arg.description);
                quote! {
                    #description
                    #name: #arg_type
                }
            })
            .collect::<Vec<_>>();

        let trait_args = args_data
            .iter()
            .map(|arg| {
                let name = ident(&arg.name);
                let arg_type = &arg.trait_type;
                quote! { #name: #arg_type }
            })
            .collect::<Vec<_>>();

        let params = args_data
            .iter()
            .map(|arg| {
                let name = ident(&arg.name);
                if let Some(default_value) = &arg.default_value {
                    quote! {
                        #name.unwrap_or_else(|| #default_value)
                    }
                } else {
                    quote! { #name }
                }
            })
            .collect::<Vec<_>>();

        FieldTokens {
            name,
            macro_args,
            trait_args,
            field_type,
            field_method,
            params,
            description: &field.description,
            type_kind,
            inner_type,
            deprecation,
        }
    }

    fn gen_enum_value(&mut self, enum_value: &'doc EnumValue) -> TokenStream {
        let graphql_name = &enum_value.name;
        let name = to_enum_name(&graphql_name);
        let description = doc_tokens(&enum_value.description);

        let deprecation = quote_deprecation(&self.parse_directives(enum_value));

        quote! {
            #[allow(missing_docs)]
            #[graphql(name=#graphql_name)]
            #deprecation
            #description
            #name,
        }
    }

    fn quote_value(&mut self, value: &Value, type_name: &str, pos: Pos) -> TokenStream {
        match value {
            Value::Float(inner) => quote! { #inner },
            Value::Int(inner) => {
                let number = inner
                    .as_i64()
                    .expect("failed to convert default number argument to i64");
                let number =
                    i32_from_i64(number).expect("failed to convert default number argument to i64");
                quote! { #number }
            }
            Value::String(inner) => quote! { #inner.to_string() },
            Value::Boolean(inner) => quote! { #inner },

            Value::Enum(variant_name) => {
                let type_name = to_enum_name(type_name);
                let variant_name = to_enum_name(variant_name);
                quote! { #type_name::#variant_name }
            }

            Value::List(list) => {
                let mut acc = quote! { let mut vec = Vec::new(); };
                for value in list {
                    let value_quoted = self.quote_value(value, type_name, pos);
                    acc.extend(quote! { vec.push(#value_quoted); });
                }
                acc.extend(quote! { vec });
                quote! { { #acc } }
            }

            Value::Object(map) => self.quote_object_value(map, type_name, pos),

            Value::Variable(_) => {
                self.emit_non_fatal_error(pos, ErrorKind::VariableDefaultValue);
                quote! {}
            }

            Value::Null => quote! { None },
        }
    }

    fn quote_object_value(
        &mut self,
        map: &BTreeMap<Name, Value>,
        type_name: &str,
        pos: Pos,
    ) -> TokenStream {
        let name = ident(&type_name);

        let mut fields_seen = HashSet::new();

        // Set fields given in `map`
        let mut field_assigments = map
            .iter()
            .map(|(key, value)| {
                fields_seen.insert(key);
                let field_name = ident(key.to_snake_case());

                let field_type_name = self
                    .ast_data
                    .input_object_field_type_name(&type_name, &key)
                    .unwrap();

                let value_quote = self.quote_value(value, field_type_name, pos);
                match self
                    .ast_data
                    .input_object_field_is_nullable(&type_name, &key)
                {
                    Some(true) | None => {
                        if value == &Value::Null {
                            quote! { #field_name: #value_quote }
                        } else {
                            quote! { #field_name: Some(#value_quote) }
                        }
                    }
                    Some(false) => quote! { #field_name: #value_quote },
                }
            })
            .collect::<Vec<_>>();

        // Set fields not given in map to `None`
        if let Some(fields) = self.ast_data.input_object_field_names(&type_name) {
            for field_name in fields {
                if !fields_seen.contains(field_name) {
                    let field_name = ident(field_name.to_snake_case());
                    field_assigments.push(quote! {
                        #field_name: None
                    });
                }
            }
        }

        let tokens = quote! {
            #name {
                #(#field_assigments),*,
            }
        };

        quote! { { #tokens } }
    }

    // Type according to https://graphql.org/learn/schema/#scalar-types
    pub fn graphql_scalar_type_to_rust_type(
        &mut self,
        name: &str,
        pos: Pos,
    ) -> (TokenStream, TypeKind) {
        match name {
            "Int" => (quote! { i32 }, TypeKind::Scalar),
            "Float" => (quote! { f64 }, TypeKind::Scalar),
            "String" => (quote! { String }, TypeKind::Scalar),
            "Boolean" => (quote! { bool }, TypeKind::Scalar),
            "ID" => (quote! { juniper::ID }, TypeKind::Scalar),
            "Date" => {
                if !self.is_date_scalar_defined() {
                    self.emit_fatal_error(pos, ErrorKind::DateScalarNotDefined)
                        .ok();
                }
                (quote! { chrono::naive::NaiveDate }, TypeKind::Scalar)
            }
            "DateTime" => {
                if !self.is_date_time_scalar_defined() {
                    self.emit_fatal_error(pos, ErrorKind::DateTimeScalarNotDefined)
                        .ok();
                }
                (
                    quote! { chrono::DateTime<chrono::offset::Utc> },
                    TypeKind::Scalar,
                )
            }
            "Uuid" => {
                if !self.is_uuid_scalar_defined() {
                    self.emit_fatal_error(pos, ErrorKind::UuidScalarNotDefined)
                        .ok();
                }
                (quote! { uuid::Uuid }, TypeKind::Scalar)
            }
            "Url" => {
                if !self.is_url_scalar_defined() {
                    self.emit_fatal_error(pos, ErrorKind::UrlScalarNotDefined)
                        .ok();
                }
                (quote! { url::Url }, TypeKind::Scalar)
            }
            name => {
                if self.is_scalar(name) || self.is_enum(name) {
                    (quote_ident(name.to_camel_case()), TypeKind::Scalar)
                } else {
                    (quote_ident(name.to_camel_case()), TypeKind::Type)
                }
            }
        }
    }
}

fn quote_deprecation(deprecated: &Deprecation) -> TokenStream {
    match deprecated {
        Deprecation::NoDeprecation => empty_token_stream(),
        Deprecation::Deprecated(Some(reason)) => {
            quote! { #[deprecated(note = #reason)] }
        }
        Deprecation::Deprecated(None) => {
            quote! { #[deprecated] }
        }
    }
}

impl Extend<TokenTree> for CodeGenPass<'_> {
    fn extend<T: IntoIterator<Item = TokenTree>>(&mut self, iter: T) {
        self.tokens.extend(iter);
    }
}

impl Extend<TokenStream> for CodeGenPass<'_> {
    fn extend<T: IntoIterator<Item = TokenStream>>(&mut self, iter: T) {
        self.tokens.extend(iter);
    }
}

fn to_enum_name(name: &str) -> Ident {
    ident(name.to_camel_case())
}

fn trait_map_for_struct_name(struct_name: &Ident) -> Ident {
    ident(format!("{}Fields", struct_name))
}

fn gen_field(
    field: &FieldTokens,
    struct_name: &Ident,
    trait_name: &Ident,
    error_type: &syn::Type,
) -> TokenStream {
    let field_name = &field.name;
    let field_type = &field.field_type;
    let args = &field.macro_args;

    let body = gen_field_body(&field, &quote! { &self }, struct_name, trait_name);

    let description = field
        .description
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_else(String::new);

    let all_args = to_field_args_list(args);

    let deprecation = &field.deprecation;

    quote! {
        #[doc = #description]
        #deprecation
        field #field_name(#all_args) -> std::result::Result<#field_type, #error_type> {
            #body
        }
    }
}

fn gen_field_body(
    field: &FieldTokens,
    self_tokens: &TokenStream,
    struct_name: &Ident,
    trait_name: &Ident,
) -> TokenStream {
    let field_method = &field.field_method;
    let params = &field.params;

    match field.type_kind {
        TypeKind::Scalar => {
            quote! {
                <#struct_name as self::#trait_name>::#field_method(#self_tokens, &executor, #(#params),*)
            }
        }
        TypeKind::Type => {
            let query_trail_type = ident(&field.inner_type);
            quote! {
                let look_ahead = executor.look_ahead();
                let trail = look_ahead.make_query_trail::<#query_trail_type>();
                <#struct_name as self::#trait_name>::#field_method(#self_tokens, &executor, &trail, #(#params),*)
            }
        }
    }
}

fn to_field_args_list(args: &[TokenStream]) -> TokenStream {
    if args.is_empty() {
        quote! { &executor }
    } else {
        quote! { &executor, #(#args),* }
    }
}

fn empty_token_stream() -> TokenStream {
    quote! {}
}

#[derive(Debug, Clone)]
struct FieldTokens<'a> {
    name: Ident,
    macro_args: Vec<TokenStream>,
    trait_args: Vec<TokenStream>,
    field_type: TokenStream,
    field_method: Ident,
    params: Vec<TokenStream>,
    description: &'a Option<String>,
    type_kind: TypeKind,
    inner_type: Name,
    deprecation: TokenStream,
}

struct FieldArgument<'a> {
    name: Name,
    macro_type: TokenStream,
    trait_type: TokenStream,
    default_value: Option<TokenStream>,
    description: &'a Option<String>,
}

// This can also be with TryInto, but that requires 1.34
#[allow(clippy::cast_lossless)]
fn i32_from_i64(i: i64) -> Option<i32> {
    if i > std::i32::MAX as i64 {
        None
    } else {
        Some(i as i32)
    }
}

enum FieldTypeDestination {
    Argument,
    Return(FieldArguments),
}

fn doc_tokens(doc: &Option<String>) -> TokenStream {
    if let Some(doc) = doc {
        quote! {
            #[doc = #doc]
        }
    } else {
        quote! {}
    }
}

struct FieldNameCaseValidator<'pass, 'doc> {
    pass: &'pass mut CodeGenPass<'doc>,
}

impl<'pass, 'doc> FieldNameCaseValidator<'pass, 'doc> {
    fn new(pass: &'pass mut CodeGenPass<'doc>) -> Self {
        Self { pass }
    }
}

impl<'pass, 'doc> SchemaVisitor<'doc> for FieldNameCaseValidator<'pass, 'doc> {
    fn visit_object_type(&mut self, ty: &'doc schema::ObjectType) {
        self.validate_fields(&ty.fields);
    }

    fn visit_interface_type(&mut self, ty: &'doc schema::InterfaceType) {
        self.validate_fields(&ty.fields);
    }

    fn visit_input_object_type(&mut self, ty: &'doc schema::InputObjectType) {
        for field in &ty.fields {
            self.validate_field(&field.name, field.position);
        }
    }
}

impl FieldNameCaseValidator<'_, '_> {
    fn validate_fields(&mut self, fields: &[Field]) {
        for field in fields {
            self.validate_field(&field.name, field.position);
        }
    }

    fn validate_field(&mut self, name: &str, pos: Pos) {
        if is_snake_case(name) {
            self.pass
                .emit_non_fatal_error(pos, ErrorKind::FieldNameInSnakeCase);
        }
    }
}

fn is_snake_case(s: &str) -> bool {
    s.contains('_') && s.to_snake_case() == s
}

trait ParseDirective<'doc, T> {
    type Output;

    fn parse_directives(&mut self, input: &'doc T) -> Self::Output;
}

impl<'doc> ParseDirective<'doc, Field> for CodeGenPass<'doc> {
    type Output = FieldArguments;

    fn parse_directives(&mut self, input: &'doc Field) -> Self::Output {
        let mut ownership = Ownership::default();
        let mut deprecated = None::<Deprecation>;

        for dir in &input.directives {
            match JuniperDirective::<Ownership>::from_directive(dir) {
                Ok(x) => {
                    ownership = x.args;
                    continue;
                }
                Err(_) => {}
            }

            match Deprecation::from_directive(dir) {
                Ok(x) => {
                    deprecated = Some(x);
                    continue;
                }
                Err(_) => {}
            }

            self.emit_non_fatal_error(
                dir.position,
                ErrorKind::UnknownDirective(vec![
                    "`@deprecated`",
                    "`@juniper(ownership: \"owned\")`",
                ]),
            );
        }

        FieldArguments {
            ownership,
            deprecated,
        }
    }
}

impl<'doc> ParseDirective<'doc, EnumValue> for CodeGenPass<'doc> {
    type Output = Deprecation;

    fn parse_directives(&mut self, input: &'doc EnumValue) -> Self::Output {
        let mut deprecated = Deprecation::default();

        for dir in &input.directives {
            match Deprecation::from_directive(dir) {
                Ok(x) => {
                    deprecated = x;
                }
                Err(err) => {
                    self.emit_non_fatal_error(dir.position, err);
                }
            }
        }

        deprecated
    }
}

macro_rules! supports_no_directives {
    ($ty:ty) => {
        impl<'doc> ParseDirective<'doc, $ty> for CodeGenPass<'doc> {
            type Output = ();

            fn parse_directives(&mut self, input: &'doc $ty) -> Self::Output {
                for directive in &input.directives {
                    self.emit_non_fatal_error(
                        directive.position,
                        ErrorKind::UnknownDirective(vec![]),
                    );
                }
            }
        }
    };
}

supports_no_directives!(SchemaDefinition);
supports_no_directives!(ScalarType);
supports_no_directives!(ObjectType);
supports_no_directives!(InterfaceType);
supports_no_directives!(UnionType);
supports_no_directives!(EnumType);
supports_no_directives!(InputObjectType);
supports_no_directives!(InputValue);

#[cfg(test)]
mod test {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_is_snake_case() {
        assert!(is_snake_case("foo_bar"));
        assert!(is_snake_case("foo_bar_baz"));

        assert!(!is_snake_case("foo"));
        assert!(!is_snake_case("fooBar"));
        assert!(!is_snake_case("FooBar"));
    }
}
