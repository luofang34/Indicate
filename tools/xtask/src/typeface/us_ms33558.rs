//! `us-ms33558`: an original stroke face drawn from the MS33558 (ASG)
//! drawing of aircraft instrument dial numerals and letters, to the
//! dimensions MIL-M-18012B gives.
//!
//! The geometry is a centreline per character, authored for this
//! repository; no font data was copied. MS33558C is a US Government
//! work released under Distribution Statement A, and it is the source of
//! the letterforms only: a plain bar "1", a closed "4", an uncrossed
//! "7", a round-topped "3", an "O" wider than "0", an "M" with vertical
//! sides and a "G" with a spur. MIL-M-18012B supplies the proportions:
//! a character is 3/5 of its height wide measured over the outside of
//! its strokes, "4" is one stroke wider, "1" and "I" are one stroke
//! wide, "M" and "W" are a fifth wider, and the stroke is 1/8 to 1/6 of
//! the height.
//!
//! Units: the cap height is 1000 and the default pen 140 (H/7, inside
//! the standard's range). A regular glyph's outside edges span x 100 to
//! 700 and y 0 to 1000, so its centreline spans x 170 to 630 and y 70
//! to 930, and its advance of 800 leaves 200 of clear space between
//! characters. The standard omits punctuation; the symbols here are
//! drawn in the same construction.

use indicate_instrument_glyphs::typeface::{FaceDef, GlyphDef, Kern, Seg, Shape, StrokeDef};

const fn line(x: i16, y: i16) -> Seg {
    Seg::Line([x, y])
}

const fn arc(c: [i16; 2], r: [u16; 2], start: i16, sweep: i16) -> Seg {
    Seg::Arc {
        center: c,
        radii: r,
        start,
        sweep,
    }
}

const fn st<'a>(start: [i16; 2], segments: &'a [Seg]) -> StrokeDef<'a> {
    StrokeDef { start, segments }
}

const fn g<'a>(ch: char, advance: u16, strokes: &'a [StrokeDef<'a>]) -> GlyphDef<'a> {
    GlyphDef {
        ch,
        alternate: 0,
        advance,
        shape: Shape::Stroke(strokes),
    }
}

/// The regular advance: 600 of ink and 200 of clear space.
const ADV: u16 = 800;
/// A one-stroke-wide character: 140 of ink and 200 of clear space.
const NARROW: u16 = 340;
/// "M" and "W": a fifth wider.
const WIDE: u16 = 920;

/// The flagged "1" of some 1940s dials, as an alternate.
pub const ONE_FLAG: u8 = 1;

const GLYPHS: &[GlyphDef<'static>] = &[
    g(' ', 400, &[]),
    g(
        '(',
        525,
        &[st([355, 985], &[arc([520, 500], [330, 560], 1200, 1200)])],
    ),
    g(
        ')',
        505,
        &[st([170, 985], &[arc([5, 500], [330, 560], 600, -1200)])],
    ),
    g(
        '+',
        ADV,
        &[
            st([170, 450], &[line(630, 450)]),
            st([400, 220], &[line(400, 680)]),
        ],
    ),
    g(',', NARROW, &[st([180, 70], &[line(130, -110)])]),
    g('-', 700, &[st([170, 450], &[line(530, 450)])]),
    g('.', NARROW, &[st([170, 70], &[])]),
    g('/', ADV, &[st([170, 70], &[line(630, 930)])]),
    g(
        '0',
        ADV,
        &[st([630, 500], &[arc([400, 500], [230, 430], 0, 3600)])],
    ),
    g('1', NARROW, &[st([170, 70], &[line(170, 930)])]),
    GlyphDef {
        ch: '1',
        alternate: ONE_FLAG,
        advance: 490,
        shape: Shape::Stroke(&[st([320, 70], &[line(320, 930), line(170, 800)])]),
    },
    g(
        '2',
        ADV,
        &[st(
            [201, 815],
            &[
                arc([400, 700], [230, 230], 1500, -1900),
                line(170, 70),
                line(630, 70),
            ],
        )],
    ),
    g(
        '3',
        ADV,
        &[st(
            [210, 823],
            &[
                arc([400, 715], [220, 215], 1500, -2400),
                arc([400, 285], [230, 215], 900, -2600),
            ],
        )],
    ),
    g(
        '4',
        940,
        &[st(
            [560, 70],
            &[line(560, 930), line(170, 330), line(770, 330)],
        )],
    ),
    g(
        '5',
        ADV,
        &[st(
            [610, 930],
            &[
                line(175, 930),
                line(170, 555),
                arc([395, 310], [235, 240], 1100, -2600),
            ],
        )],
    ),
    g(
        '6',
        ADV,
        &[
            st([515, 872], &[arc([400, 500], [230, 430], 600, 1500)]),
            st([630, 290], &[arc([400, 290], [230, 220], 0, 3600)]),
        ],
    ),
    g(
        '7',
        ADV,
        &[st([170, 930], &[line(630, 930), line(300, 70)])],
    ),
    g(
        '8',
        ADV,
        &[
            st([600, 725], &[arc([400, 725], [200, 205], 0, 3600)]),
            st([630, 285], &[arc([400, 285], [230, 215], 0, 3600)]),
        ],
    ),
    g(
        '9',
        ADV,
        &[
            st([630, 710], &[arc([400, 710], [230, 220], 0, 3600)]),
            st([285, 128], &[arc([400, 500], [230, 430], 2400, 1500)]),
        ],
    ),
    g(
        '=',
        ADV,
        &[
            st([170, 330], &[line(630, 330)]),
            st([170, 570], &[line(630, 570)]),
        ],
    ),
    g(
        'A',
        ADV,
        &[
            st([170, 70], &[line(400, 930), line(630, 70)]),
            st([240, 330], &[line(560, 330)]),
        ],
    ),
    g(
        'B',
        ADV,
        &[
            st(
                [170, 70],
                &[
                    line(170, 930),
                    line(450, 930),
                    arc([450, 715], [180, 215], 900, -1800),
                    line(170, 500),
                ],
            ),
            st(
                [170, 500],
                &[
                    line(470, 500),
                    arc([470, 285], [160, 215], 900, -1800),
                    line(170, 70),
                ],
            ),
        ],
    ),
    g(
        'C',
        ADV,
        &[st([637, 715], &[arc([420, 500], [250, 430], 300, 3000)])],
    ),
    g(
        'D',
        ADV,
        &[st(
            [170, 70],
            &[
                line(170, 930),
                line(360, 930),
                arc([360, 500], [270, 430], 900, -1800),
                line(170, 70),
            ],
        )],
    ),
    g(
        'E',
        ADV,
        &[
            st([630, 930], &[line(170, 930), line(170, 70), line(630, 70)]),
            st([170, 500], &[line(560, 500)]),
        ],
    ),
    g(
        'F',
        ADV,
        &[
            st([630, 930], &[line(170, 930), line(170, 70)]),
            st([170, 500], &[line(560, 500)]),
        ],
    ),
    g(
        'G',
        ADV,
        &[st(
            [563, 804],
            &[
                arc([400, 500], [230, 430], 450, 2850),
                line(630, 430),
                line(430, 430),
            ],
        )],
    ),
    g(
        'H',
        ADV,
        &[
            st([170, 70], &[line(170, 930)]),
            st([630, 70], &[line(630, 930)]),
            st([170, 500], &[line(630, 500)]),
        ],
    ),
    g('I', NARROW, &[st([170, 70], &[line(170, 930)])]),
    g(
        'J',
        ADV,
        &[st(
            [630, 930],
            &[line(630, 280), arc([400, 280], [230, 210], 0, -1800)],
        )],
    ),
    g(
        'K',
        ADV,
        &[
            st([170, 70], &[line(170, 930)]),
            st([630, 930], &[line(170, 380)]),
            st([330, 571], &[line(630, 70)]),
        ],
    ),
    g('L', ADV, &[st([170, 930], &[line(170, 70), line(630, 70)])]),
    g(
        'M',
        WIDE,
        &[st(
            [170, 70],
            &[
                line(170, 930),
                line(460, 330),
                line(750, 930),
                line(750, 70),
            ],
        )],
    ),
    g(
        'N',
        ADV,
        &[st(
            [170, 70],
            &[line(170, 930), line(630, 70), line(630, 930)],
        )],
    ),
    g(
        'O',
        900,
        &[st([730, 500], &[arc([450, 500], [280, 430], 0, 3600)])],
    ),
    g(
        'P',
        ADV,
        &[st(
            [170, 70],
            &[
                line(170, 930),
                line(430, 930),
                arc([430, 710], [200, 220], 900, -1800),
                line(170, 490),
            ],
        )],
    ),
    g(
        'Q',
        900,
        &[
            st([730, 500], &[arc([450, 500], [280, 430], 0, 3600)]),
            st([560, 240], &[line(760, 40)]),
        ],
    ),
    g(
        'R',
        ADV,
        &[
            st(
                [170, 70],
                &[
                    line(170, 930),
                    line(430, 930),
                    arc([430, 710], [200, 220], 900, -1800),
                    line(170, 490),
                ],
            ),
            st([400, 490], &[line(630, 70)]),
        ],
    ),
    g(
        'S',
        ADV,
        &[st(
            [607, 789],
            &[
                arc([400, 715], [220, 215], 200, 2500),
                arc([400, 285], [230, 215], 900, -2500),
            ],
        )],
    ),
    g(
        'T',
        ADV,
        &[
            st([170, 930], &[line(630, 930)]),
            st([400, 930], &[line(400, 70)]),
        ],
    ),
    g(
        'U',
        ADV,
        &[st(
            [170, 930],
            &[
                line(170, 300),
                arc([400, 300], [230, 230], 1800, 1800),
                line(630, 930),
            ],
        )],
    ),
    g(
        'V',
        ADV,
        &[st([170, 930], &[line(400, 70), line(630, 930)])],
    ),
    g(
        'W',
        WIDE,
        &[st(
            [170, 930],
            &[line(300, 70), line(460, 700), line(620, 70), line(750, 930)],
        )],
    ),
    g(
        'X',
        ADV,
        &[
            st([170, 930], &[line(630, 70)]),
            st([170, 70], &[line(630, 930)]),
        ],
    ),
    g(
        'Y',
        ADV,
        &[
            st([170, 930], &[line(400, 480), line(630, 930)]),
            st([400, 480], &[line(400, 70)]),
        ],
    ),
    g(
        'Z',
        ADV,
        &[st(
            [170, 930],
            &[line(630, 930), line(170, 70), line(630, 70)],
        )],
    ),
    g(
        '°',
        570,
        &[st([400, 820], &[arc([290, 820], [110, 110], 0, 3600)])],
    ),
    g(
        '×',
        ADV,
        &[
            st([190, 240], &[line(610, 660)]),
            st([190, 660], &[line(610, 240)]),
        ],
    ),
];

const KERNING: &[Kern] = &[
    Kern {
        left: '7',
        right: '.',
        adjust: -120,
    },
    Kern {
        left: 'L',
        right: 'T',
        adjust: -80,
    },
    Kern {
        left: 'L',
        right: 'V',
        adjust: -80,
    },
    Kern {
        left: 'P',
        right: '.',
        adjust: -80,
    },
    Kern {
        left: 'T',
        right: '.',
        adjust: -100,
    },
    Kern {
        left: 'V',
        right: '.',
        adjust: -80,
    },
];

/// The face definition.
pub const FACE: FaceDef<'static> = FaceDef {
    id: "us-ms33558",
    units_per_em: 1250,
    cap_height: 1000,
    ascender: 1000,
    descender: -250,
    default_pen: 140,
    alternates: &[(ONE_FLAG, "one.flag")],
    glyphs: GLYPHS,
    kerning: KERNING,
};
