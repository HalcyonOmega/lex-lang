#[test]
fn enum_is_keyword() {
    let (toks, diags) = lex::lexer::lex("enum x { A; }");
    assert!(diags.is_empty(), "{diags:?}");
    assert!(matches!(toks[0].kind, lex::lexer::TokKind::KwEnum), "{:?}", toks[0].kind);
}
