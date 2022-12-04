// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2022 Stacks Open Internet Foundation
// Copyright (C) 2022 Jude Nelson
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::fs;
use crate::ui::Renderer;
use crate::vm::ClarityVM;

#[test]
fn test_render_codec() {
    let txt = "hello world!";
    let renderer = Renderer::new(1024);
    let bytes = renderer.encode_bytes(txt.as_bytes()).unwrap();
    
    let mut bytes_decoded = vec![];
    renderer.decode(&mut &bytes[..], &mut bytes_decoded).unwrap();
    let s = std::str::from_utf8(&bytes_decoded).unwrap();

    assert_eq!(&s, &txt);
}

#[test]
fn test_render_eval_hello_world() {
    let db_path = "/tmp/wrb-render-eval-hello-world";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r#"
```wrb:main
(print "hello world")
(define-public (foo)
    (ok (print "foo")))
```
Hello Markdown!
```wrb
(foo)
```
    "#;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}

#[test]
fn test_render_eval_headings() {
    let db_path = "/tmp/wrb-render-eval-headings";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r##########"
```wrb:main
(print "hello world")
(define-public (foo)
    (begin
        (print "# H1")
        (print "## H2")
        (print "### H3")
        (print "#### H4")
        (print "##### H5")
        (print "###### H6")
    (ok (print "foo"))))
```
Hello Markdown!
```wrb
(foo)
```
    "##########;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}

#[test]
fn test_render_eval_block_quote() {
    let db_path = "/tmp/wrb-render-eval-block-quote";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r##########"
```wrb:main
(print "hello world")
(define-public (foo)
    (begin
        (print "> level 1")
        (print "> > level 2")
        (print ">>> level 3")
        (print ">>> > level 4")
    (ok (print "foo"))))
```
Hello Markdown!
```wrb
(foo)
```
    "##########;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}

#[test]
fn test_render_eval_code_block() {
    let db_path = "/tmp/wrb-render-eval-code-block";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r##########"
```wrb:main
(print "hello world")
(define-public (foo)
    (begin
        (print "```")
        (print "echo 'hello world'")
        (print "```")
        (print "```bash")
        (print "echo 'hello world with fence'")
        (print "```")
        (print "```wrb")
        (print "echo 'hello wrb fence'")
        (print "```")
        (print "```bash")
        (print "```inner-bash")
        (print "do nested code block fences work lol")
        (print "```")
        (print "```")
    (ok (print "foo"))))
```
Hello Markdown!
```wrb
(foo)
```
    "##########;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}

#[test]
fn test_render_eval_list() {
    let db_path = "/tmp/wrb-render-eval-list";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r##########"
```wrb:main
(print "hello world")
(define-public (foo)
    (begin
        (print "1. first item")
        (print "2. second item")
        (print "3. third item")
        (print "---")
        (print "10. tenth item")
        (print "10. eleventh item")
        ;; what the fuck?
        (print "  30. nested item 30")
        (print "  30. nested item 31")
        (print "  30. nested item 32")
        (print "10. twelfth item")
    (ok (print "foo"))))
```
Hello Markdown!
```wrb
(foo)
```
    "##########;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}

#[test]
fn test_render_eval_table() {
    let db_path = "/tmp/wrb-render-eval-list";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r##########"
```wrb:main
(print "hello world")
(define-public (foo)
    (begin
        (print "")
        (print "before the table")
        (print "")
        (print "| Syntax      | Description | Test Text     |")
        (print "| :---        |    :----:   |          ---: |")
        (print "| Header      | Title       | Here's this   |")
        (print "| Paragraph   | Text        | And more      |")
        (print "")
        (print "after the table")
        (print "")
        (print "| Syntax      | Description | Test Text     |")
        (print "| ---         |    ----     |          ---  |")
        (print "| Header      | Title       | Here's this   |")
        (print "| Paragraph   | Text        | And more      |")
        (print "")
        (print "after the second table")
        (print "")
    (ok (print "foo"))))
```
Hello Markdown!
```wrb
(foo)
```
    "##########;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}

#[test]
fn test_render_eval_formatters() {
    let db_path = "/tmp/wrb-render-eval-list";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let input = r##########"
```wrb:main
(print "hello world")
(define-public (foo)
    (begin
        (print "*emphasis*")
        (print "_emphasis_")
        (print "**bold**")
        (print "~~strikethrough~~")
    (ok (print "foo"))))
```
Hello Markdown!
```wrb
(foo)
```
    "##########;
    
    let mut renderer = Renderer::new(1024);
    let s = renderer.eval_to_string(&mut vm, &input).unwrap();
    eprintln!("<<<<<\n{}>>>>>", &s);
}


