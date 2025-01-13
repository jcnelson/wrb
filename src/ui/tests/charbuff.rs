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
use crate::ui::Renderer;
use crate::vm::ClarityVM;
use std::fs;

fn render_charbuff(buff: &CharBuff) -> String {
    let scanlines = Scanline::compile(buff);
    println!("{:?}", &scanlines);
    let mut output = "".to_string();
    for sl in scanlines {
        output.push_str(&sl.into_term_code());
    }
    output
}
#[test]
fn test_render_charbuff() {
    let mut charbuff = CharBuff::new(80);
    charbuff.print_at(
        100,
        0,
        0,
        0x00000000.into(),
        0x00ffffff.into(),
        "Hello world!",
    );
    charbuff.print_at(
        101,
        1,
        10,
        0x00000000.into(),
        0x000000ff.into(),
        "Hello world in blue!",
    );
    charbuff.print_at(
        102,
        2,
        20,
        0x00000000.into(),
        0x0000ff00.into(),
        "Hello world in green!",
    );
    charbuff.print_at(
        103,
        3,
        30,
        0x00000000.into(),
        0x00ff0000.into(),
        "Hello world in red!",
    );
    charbuff.print_at(
        104,
        5,
        0,
        0x00000000.into(),
        0xffffffff.into(),
        "wordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrap",
    );
    charbuff.print_at(
        105,
        3,
        25,
        0x00ff0000.into(),
        0x00000000.into(),
        "overwrite",
    );

    // check contents of charbuff
    assert_eq!(
        charbuff,
        CharBuff {
            num_cols: 80,
            cells: vec![
                CharCell::Fill {
                    element_id: 100,
                    value: 'H',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 100,
                    value: '!',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Fill {
                    element_id: 101,
                    value: 'H',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'i',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'b',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'u',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    element_id: 101,
                    value: '!',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Fill {
                    element_id: 102,
                    value: 'H',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'i',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'g',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    element_id: 102,
                    value: '!',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Fill {
                    element_id: 105,
                    value: 'o',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'v',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'e',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'r',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'w',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'r',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'i',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 't',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 105,
                    value: 'e',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'i',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    element_id: 103,
                    value: '!',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Blank,
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    element_id: 104,
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                }
            ]
        }
    );

    /*
    let scanlines = Scanline::compile(&charbuff);
    assert_eq!(
        scanlines,
        vec![
            Scanline::ClearLine,
            Scanline::FgColor(Color {
                r: 255,
                g: 255,
                b: 255
            }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("Hello world!".into()),
            Scanline::ResetColor,
            Scanline::Text("                                                                    ".into()),
            Scanline::Newline,
            Scanline::ResetColor,
            Scanline::ClearLine,
            Scanline::Text("          ".into()),
            Scanline::FgColor(Color { r: 0, g: 0, b: 255 }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("Hello world in blue!".into()),
            Scanline::ResetColor,
            Scanline::Text("                                                  ".into()),
            Scanline::Newline,
            Scanline::ResetColor,
            Scanline::ClearLine,
            Scanline::Text("                    ".into()),
            Scanline::FgColor(Color { r: 0, g: 255, b: 0 }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("Hello world in green!".into()),
            Scanline::ResetColor,
            Scanline::Text("                                       ".into()),
            Scanline::Newline,
            Scanline::ResetColor,
            Scanline::ClearLine,
            Scanline::Text("                         ".into()),
            Scanline::FgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::BgColor(Color { r: 255, g: 0, b: 0 }),
            Scanline::Text("overwrite".into()),
            Scanline::FgColor(Color { r: 255, g: 0, b: 0 }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("o world in red!".into()),
            Scanline::ResetColor,
            Scanline::Text("                               ".into()),
            Scanline::Newline,
            Scanline::ResetColor,
            Scanline::ClearLine,
            Scanline::Text(
                "                                                                                ".into()
            ),
            Scanline::Newline,
            Scanline::ResetColor,
            Scanline::ClearLine,
            Scanline::FgColor(Color {
                r: 255,
                g: 255,
                b: 255
            }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text(
                "wordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrap".into()
            ),
            Scanline::Newline,
            Scanline::ResetColor,
            Scanline::ClearLine,
            Scanline::FgColor(Color {
                r: 255,
                g: 255,
                b: 255
            }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("wordwrap".into()),
            Scanline::ResetColor
        ]
    );

    let mut output = "".to_string();
    for sl in scanlines {
        output.push_str(&sl.into_term_code());
    }
    println!("{}", &output);
    */
}
