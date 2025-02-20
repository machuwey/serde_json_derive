use cairo_lang_macro::{derive_macro, ProcMacroResult, TokenStream};
use cairo_lang_parser::utils::SimpleParserDatabase;
use cairo_lang_syntax::node::kind::SyntaxKind::{TerminalStruct, TokenIdentifier};


#[derive_macro]
pub fn json_deserialize(token_stream: TokenStream) -> ProcMacroResult {
    let db = SimpleParserDatabase::default();
    let (parsed, _diag) = db.parse_virtual_with_diagnostics(token_stream);

    // Extract struct name
    let mut nodes = parsed.descendants(&db);
    let mut struct_name = String::new();
    for node in nodes.by_ref() {
        if node.kind(&db) == TerminalStruct {
            struct_name = nodes
                .find(|node| node.kind(&db) == TokenIdentifier)
                .unwrap()
                .get_text(&db);
            break;
        }
    }

    // Extract field names
    let mut field_names: Vec<String> = Vec::new();
    for node in parsed.descendants(&db) {
        if node.kind(&db) == TokenIdentifier && node.parent().unwrap().kind(&db) != TerminalStruct {
            field_names.push(node.get_text(&db));
        }
    }

    // Generate field parsing code
    let mut field_parsing = String::new();
    let mut validation = String::new();
    for field in &field_names {
        field_parsing.push_str(&format!(
            r#"
            else if field_name == "{0}" {{
                match json_parser::parse_string(data, ref pos) {{
                    Result::Ok(value) => {{
                        result.{0} = value;
                        {0}_parsed = true;
                    }},
                    Result::Err(e) => {{
                        error = e;
                        success = false;
                        break;
                    }}
                }}
            }}
            "#,
            field
        ));
        validation.push_str(&format!("{}_parsed && ", field));
    }
    validation = validation.trim_end_matches(" && ").to_string();

    // Generate the full implementation
    let generated_code = format!(
        r#"
        impl {0}JsonDeserialize of serde_json::JsonDeserialize<{0}> {{
            fn deserialize(data: @ByteArray, ref pos: usize) -> Result<{0}, ByteArray> {{
                let mut result = {0} {{ {1} }};
                let mut success = true;
                let mut error: ByteArray = "";
                {2}

                serde_json::json_parser::skip_whitespace(data, ref pos);
                if pos >= data.len() || data[pos] != 123_u8 {{
                    return Result::Err("Expected object");
                }}
                pos += 1;

                loop {{
                    serde_json::json_parser::skip_whitespace(data, ref pos);
                    if pos >= data.len() || data[pos] == 125_u8 {{
                        break;
                    }}
                    match serde_json::json_parser::parse_string(data, ref pos) {{
                        Result::Ok(field_name) => {{
                            serde_json::json_parser::skip_whitespace(data, ref pos);
                            if pos >= data.len() || data[pos] != 58_u8 {{
                                error = "Expected ':'";
                                success = false;
                                break;
                            }}
                            pos += 1;
                            if false {{}}
                            {3}
                            else {{
                                match serde_json::json_parser::parse_string(data, ref pos) {{
                                    Result::Ok(_) => {{}},
                                    Result::Err(e) => {{
                                        error = e;
                                        success = false;
                                        break;
                                    }}
                                }}
                            }}
                        }},
                        Result::Err(e) => {{
                            error = e;
                            success = false;
                            break;
                        }}
                    }};
                    serde_json::json_parser::skip_whitespace(data, ref pos);
                    if pos < data.len() && data[pos] == 44_u8 {{
                        pos += 1;
                    }}
                }};

                if !success {{
                    Result::Err(error)
                }} else if !({4}) {{
                    Result::Err("Missing required field")
                }} else if pos >= data.len() || data[pos] != 125_u8 {{
                    Result::Err("Expected closing brace")
                }} else {{
                    pos += 1;
                    Result::Ok(result)
                }}
            }}
        }}
        "#,
        struct_name,
        field_names.iter().map(|f| format!("{}: \"\"", f)).collect::<Vec<_>>().join(", "),
        field_names.iter().map(|f| format!("let mut {}_parsed = false;", f)).collect::<Vec<_>>().join("\n                "),
        field_parsing,
        validation
    );

    ProcMacroResult::new(TokenStream::new(generated_code))
}