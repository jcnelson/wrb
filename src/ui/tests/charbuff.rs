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
    charbuff.print_at(0, 0, 0x00000000.into(), 0x00ffffff.into(), "Hello world!");
    charbuff.print_at(
        10,
        1,
        0x00000000.into(),
        0x000000ff.into(),
        "Hello world in blue!",
    );
    charbuff.print_at(
        20,
        2,
        0x00000000.into(),
        0x0000ff00.into(),
        "Hello world in green!",
    );
    charbuff.print_at(
        30,
        3,
        0x00000000.into(),
        0x00ff0000.into(),
        "Hello world in red!",
    );
    charbuff.print_at(
        0,
        5,
        0x00000000.into(),
        0xffffffff.into(),
        "wordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrapwordwrap",
    );
    charbuff.print_at(25, 3, 0x00ff0000.into(), 0x00000000.into(), "overwrite");

    // check contents of charbuff
    assert_eq!(
        charbuff,
        CharBuff {
            num_cols: 80,
            cells: vec![
                CharCell::Fill {
                    value: 'H',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
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
                    value: 'H',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'i',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'b',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'u',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 255 }
                },
                CharCell::Fill {
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
                    value: 'H',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'i',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'g',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 0, g: 255, b: 0 }
                },
                CharCell::Fill {
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
                    value: 'o',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'v',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'i',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 't',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 255, g: 0, b: 0 },
                    fg: Color { r: 0, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'l',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'i',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'n',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: ' ',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'e',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color { r: 255, g: 0, b: 0 }
                },
                CharCell::Fill {
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
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'p',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'o',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'd',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'w',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'r',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
                    value: 'a',
                    bg: Color { r: 0, g: 0, b: 0 },
                    fg: Color {
                        r: 255,
                        g: 255,
                        b: 255
                    }
                },
                CharCell::Fill {
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
            Scanline::ClearLine,
            Scanline::Text("          ".into()),
            Scanline::FgColor(Color { r: 0, g: 0, b: 255 }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("Hello world in blue!".into()),
            Scanline::ResetColor,
            Scanline::Text("                                                  ".into()),
            Scanline::Newline,
            Scanline::ClearLine,
            Scanline::Text("                    ".into()),
            Scanline::FgColor(Color { r: 0, g: 255, b: 0 }),
            Scanline::BgColor(Color { r: 0, g: 0, b: 0 }),
            Scanline::Text("Hello world in green!".into()),
            Scanline::ResetColor,
            Scanline::Text("                                       ".into()),
            Scanline::Newline,
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
            Scanline::ClearLine,
            Scanline::Text(
                "                                                                                ".into()
            ),
            Scanline::Newline,
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
}
