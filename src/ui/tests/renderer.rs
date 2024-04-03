// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020-2023 Stacks Open Internet Foundation
// Copyright (C) 2023 Jude Nelson
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

use crate::core;
use crate::ui::Renderer;
use crate::vm::ClarityVM;
use std::fs;

use stacks_common::util::hash::to_hex;

#[test]
fn test_render_codec() {
    core::init(false, "localhost", 20443);

    let txt = "hello world!";
    let renderer = Renderer::new(1024);
    let bytes = Renderer::encode_bytes(txt.as_bytes()).unwrap();

    let mut bytes_decoded = vec![];
    renderer
        .decode(&mut &bytes[..], &mut bytes_decoded)
        .unwrap();
    let s = std::str::from_utf8(&bytes_decoded).unwrap();

    assert_eq!(&s, &txt);
}

#[test]
fn test_render_viewports_raw_text() {
    core::init(false, "localhost", 20443);

    let db_path = "/tmp/wrb-render-eval-hello-world";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }

    let code = r#"
(wrb-root u80 u60)
(wrb-viewport u0 u5 u5 u25 u25)
(wrb-viewport u1 u20 u20 u25 u25)
(wrb-viewport u2 u35 u35 u25 u25)
(wrb-raw-txt u0 u0 u0 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u1 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u2 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u3 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u4 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u5 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u6 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u7 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u8 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u9 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u10 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u11 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u12 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u13 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u14 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u15 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u16 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u17 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u18 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")
(wrb-raw-txt u0 u0 u19 u0 (buff-to-uint-be 0x0000ff) u"hello world blue")

(wrb-raw-txt u1 u0 u0 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u1 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u2 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u3 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u4 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u5 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u6 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u7 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u8 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u9 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u10 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u11 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u12 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u13 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u14 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u15 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u16 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u17 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u18 u0 (buff-to-uint-be 0x00ff00) u"hello world green")
(wrb-raw-txt u1 u0 u19 u0 (buff-to-uint-be 0x00ff00) u"hello world green")

(wrb-raw-txt u2 u0 u0 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u1 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u2 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u3 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u4 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u5 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u6 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u7 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u8 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u9 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u10 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u11 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u12 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u13 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u14 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u15 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u16 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u17 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u18 u0 (buff-to-uint-be 0xff0000) u"hello world red")
(wrb-raw-txt u2 u0 u19 u0 (buff-to-uint-be 0xff0000) u"hello world red")
"#;
    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_string(&mut vm, &bytes).unwrap();
    println!("{}", &s); 
    
    let mut vm = ClarityVM::new(db_path, "foo-test.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();
    println!("{}", &s);
}

#[test]
fn test_render_viewports_wrapped_text() {
    core::init(false, "localhost", 20443);

    let db_path = "/tmp/wrb-render-eval-hello-world-wrapped";
    if fs::metadata(&db_path).is_ok() {
        fs::remove_dir_all(&db_path).unwrap();
    }
    
    let code = r#"
(wrb-root u80 u60)
(wrb-viewport u0 u5 u5 u25 u25)
(wrb-viewport u1 u20 u20 u25 u25)
(wrb-viewport u2 u35 u35 u25 u25)

(wrb-raw-print u0 none u0 (buff-to-uint-be 0x0000ff) u"This is the blue song that never ends.  ")
(wrb-raw-print u1 none u0 (buff-to-uint-be 0x00ff00) u"This is the green song that never ends.  ")
(wrb-raw-print u2 none u0 (buff-to-uint-be 0xff0000) u"This is the red song that never ends.  ")

(wrb-raw-println u0 none u0 (buff-to-uint-be 0x0000ff) u"Yes, it goes on and on, my friends.")
(wrb-raw-println u1 none u0 (buff-to-uint-be 0x00ff00) u"Yes, it goes on and on, my friends.")
(wrb-raw-println u2 none u0 (buff-to-uint-be 0xff0000) u"Yes, it goes on and on, my friends.")

(wrb-raw-print u0 none u0 (buff-to-uint-be 0x0000ff) u"Some people started signing it, not knowing what it was...")
(wrb-raw-print u1 none u0 (buff-to-uint-be 0x00ff00) u"Some people started signing it, not knowing what it was...")
(wrb-raw-print u2 none u0 (buff-to-uint-be 0xff0000) u"Some people started singing it, not knowing what it was...")
"#;

    let bytes = Renderer::encode_bytes(code.as_bytes()).unwrap();

    let mut vm = ClarityVM::new(db_path, "foo.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_string(&mut vm, &bytes).unwrap();
    println!("{}", &s);
    
    let mut vm = ClarityVM::new(db_path, "foo-test.btc").unwrap();
    let mut renderer = Renderer::new(1_000_000_000);
    let s = renderer.eval_to_text(&mut vm, &bytes).unwrap();

    assert_eq!(s,
"                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
     This is the blue song                                                      
     that never ends.  Yes,                                                     
     it goes on and on, my                                                      
     friends.                                                                   
     Some people started                                                        
     signing it, not knowing                                                    
     what it was...                                                             
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                    This is the green song                                      
                    that never ends.  Yes,                                      
                    it goes on and on, my                                       
                    friends.                                                    
                    Some people started                                         
                    signing it, not knowing                                     
                    what it was...                                              
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                   This is the red song                         
                                   that never ends.  Yes,                       
                                   it goes on and on, my                        
                                   friends.                                     
                                   Some people started                          
                                   singing it, not knowing                      
                                   what it was...                               
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                
                                                                                ");
}

