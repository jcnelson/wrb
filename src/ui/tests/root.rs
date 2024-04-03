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
use crate::ui::charbuff::{CharBuff, CharCell, Color};
use crate::ui::scanline::Scanline;
use crate::ui::root::Root;
use crate::ui::viewport::Viewport;

#[test]
fn test_zbuff() {
    let mut red_viewport = Viewport::new(0, 5, 5, 20, 20);
    let mut green_viewport = Viewport::new(1, 20, 20, 20, 20);
    let mut blue_viewport = Viewport::new(2, 35, 35, 20, 20);

    red_viewport.print_to(0, 0, 0x00ff0000.into(), 0x00ffffff.into(), "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr");
    green_viewport.print_to(0, 0, 0x0000ff00.into(), 0x00ffffff.into(), "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg");
    blue_viewport.print_to(0, 0, 0x000000ff.into(), 0x00ffffff.into(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    let root = Root::new(80, 60, vec![red_viewport, green_viewport, blue_viewport]);
    let zbuff = root.make_zbuff();

    let zbuff_txt = Root::dump_zbuff(&zbuff, 80);
    println!("{}", &zbuff_txt);

    assert_eq!(
        zbuff_txt,
r#"********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000000000*******************************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
*****00000000000000011111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111111111****************************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
********************11111111111111122222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
***********************************22222222222222222222*************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************
********************************************************************************"#);
}

#[test]
fn render_root() {
    let mut red_viewport = Viewport::new(0, 5, 5, 20, 20);
    let mut green_viewport = Viewport::new(1, 20, 20, 20, 20);
    let mut blue_viewport = Viewport::new(2, 35, 35, 20, 20);

    red_viewport.print_to(0, 0, 0x00ff0000.into(), 0x00ffffff.into(), "rrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrrr");
    green_viewport.print_to(0, 0, 0x0000ff00.into(), 0x00ffffff.into(), "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg");
    blue_viewport.print_to(0, 0, 0x000000ff.into(), 0x00ffffff.into(), "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb");

    let mut root = Root::new(80, 60, vec![red_viewport, green_viewport, blue_viewport]);
    
    let buff = root.refresh();
    let scanlines = Scanline::compile(&buff);
    let mut output = "".to_string();
    for sl in scanlines {
        output.push_str(&sl.into_term_code());
    }
    println!("{}", &output);
}

#[test]
fn test_wordwrap() {
    let mut viewport = Viewport::new(0, 5, 5, 10, 8);
    viewport.print(0, 0, 0.into(), 0xffffff.into(), "no wrap");
    viewport.print(0, 1, 0.into(), 0xffffff.into(), "this is going to wrap");
    viewport.print(0, 3, 0.into(), 0xffffff.into(), "biiiiiiiiiig word");
    viewport.print(0, 6, 0.into(), 0xffffff.into(), "wrap   space");

    let mut root = Root::new(25, 20, vec![viewport]);
    let buff = root.refresh();
    let scanlines = Scanline::compile(&buff);
    assert_eq!(scanlines,
		vec![
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"no wrap".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"             ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"this is ".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"            ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"going to ".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"           ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"wrap".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"                ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"biiiiiiiii".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"          ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"ig word".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"             ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"wrap   ".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"             ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"     ".into(),
			),
			Scanline::FgColor(
				Color {
					r: 255,
					g: 255,
					b: 255,
				},
			),
			Scanline::BgColor(
				Color {
					r: 0,
					g: 0,
					b: 0,
				},
			),
			Scanline::Text(
				"space".into(),
			),
			Scanline::ResetColor,
			Scanline::Text(
				"               ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::Newline,
			Scanline::ClearLine,
			Scanline::Text(
				"                         ".into(),
			),
			Scanline::ResetColor,
		]); 
    let mut output = "".to_string();
    for sl in scanlines {
        output.push_str(&sl.into_term_code());
    }
    println!("{}", &output);
}

