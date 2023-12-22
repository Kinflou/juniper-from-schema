use std::path::PathBuf;
use juniper_from_schema_code_gen::utils;
use juniper_from_schema_code_gen::CodeGen;


pub fn graphql_schema_str_from_file(schema_path: PathBuf) -> String {
    let mut builder = CodeGen::build_from_schema_file(schema_path);
    let code_gen = builder.finish();

    match code_gen.generate_code() {
        Ok(tokens) => utils::pretty_print(&tokens),
        Err(errors) => panic!("{}", errors),
    }
}



#[test]
fn from_file() {
    let stream = graphql_schema_str_from_file(
        "tests/schemas/declarations/simple.graphql".into()
    );
    println!("{}", stream)
}
