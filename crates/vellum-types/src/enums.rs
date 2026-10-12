//! Closed sets of words the file uses. Each prints as the lower-case (or kebab-case) word and nothing
//! else; `ALL` lists them in the order the docs do.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Declares a fieldless enum whose variants print as the given words, with `ALL`, `as_str` and, when
/// `default` names a variant, `Default`.
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        $name:ident $(default $default:ident)? {
            $( $(#[$vmeta:meta])* $variant:ident = $text:literal ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, JsonSchema, TS)]
        #[ts(export)]
        pub enum $name {
            $( $(#[$vmeta])* #[schemars(rename = $text)] #[ts(rename = $text)] $variant, )+
        }

        // Written out because serde also reads `{"word": null}` as a unit variant, a second
        // spelling for the same value.
        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let word = <std::borrow::Cow<'de, str> as Deserialize>::deserialize(deserializer)?;
                Self::ALL.into_iter().find(|v| v.as_str() == word).ok_or_else(|| {
                    serde::de::Error::unknown_variant(&word, &[$($text),+])
                })
            }
        }

        impl $name {
            /// Every variant, in documentation order.
            pub const ALL: [Self; [$(Self::$variant),+].len()] = [$(Self::$variant),+];

            /// The word the file uses.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $text, )+
                }
            }
        }

        $(
            impl Default for $name {
                fn default() -> Self {
                    Self::$default
                }
            }
        )?
    };
}

string_enum!(
    /// How a layer's colour combines with what is beneath it.
    BlendMode default Normal {
        Normal = "normal",
        Darken = "darken",
        Multiply = "multiply",
        ColorBurn = "color-burn",
        LinearBurn = "linear-burn",
        Lighten = "lighten",
        Screen = "screen",
        ColorDodge = "color-dodge",
        LinearDodge = "linear-dodge",
        Overlay = "overlay",
        SoftLight = "soft-light",
        HardLight = "hard-light",
        Difference = "difference",
        Exclusion = "exclusion",
        Hue = "hue",
        Saturation = "saturation",
        Color = "color",
        Luminosity = "luminosity",
    }
);

string_enum!(
    /// Which kind of node. The words are the `type` tag in the file.
    NodeKind {
        Frame = "frame",
        Group = "group",
        Rect = "rect",
        Ellipse = "ellipse",
        Polygon = "polygon",
        Star = "star",
        Line = "line",
        Vector = "vector",
        Boolean = "boolean",
        Text = "text",
        Image = "image",
        Icon = "icon",
        Instance = "instance",
        Script = "script",
        Note = "note",
        Context = "context",
        Prompt = "prompt",
        Embed = "embed",
    }
);

string_enum!(
    /// How a frame lays out its children.
    Layout default None {
        None = "none",
        Row = "row",
        Column = "column",
        Grid = "grid",
    }
);

string_enum!(
    /// Distribution along a frame's main axis.
    Justify default Start {
        Start = "start",
        Center = "center",
        End = "end",
        SpaceBetween = "space-between",
        SpaceAround = "space-around",
    }
);

string_enum!(
    /// Alignment across a frame's main axis.
    Align default Start {
        Start = "start",
        Center = "center",
        End = "end",
    }
);

string_enum!(
    /// How a vector's path decides what is inside it.
    FillRule default Nonzero {
        Nonzero = "nonzero",
        Evenodd = "evenodd",
    }
);

string_enum!(
    /// How a boolean node combines its children.
    BoolOp {
        Union = "union",
        Subtract = "subtract",
        Intersect = "intersect",
        Exclude = "exclude",
    }
);

string_enum!(
    /// Horizontal text alignment.
    TextAlign default Left {
        Left = "left",
        Center = "center",
        Right = "right",
        Justify = "justify",
    }
);

string_enum!(
    /// Vertical text alignment inside the node.
    VerticalAlign default Top {
        Top = "top",
        Middle = "middle",
        Bottom = "bottom",
    }
);

string_enum!(
    /// What changes when the text does not fit.
    TextResize default Fixed {
        AutoWidth = "auto-width",
        AutoHeight = "auto-height",
        Fixed = "fixed",
    }
);

string_enum!(
    /// Roman or italic.
    FontStyle default Normal {
        Normal = "normal",
        Italic = "italic",
    }
);

string_enum!(
    /// How an image fills its box.
    ImageFit default Fill {
        Fill = "fill",
        Fit = "fit",
        Stretch = "stretch",
    }
);

string_enum!(
    /// The shape of a gradient.
    GradientKind {
        Linear = "linear",
        Radial = "radial",
        Angular = "angular",
    }
);

string_enum!(
    /// Where a stroke sits on the edge of its shape.
    StrokeAlign default Center {
        Inside = "inside",
        Center = "center",
        Outside = "outside",
    }
);

string_enum!(
    /// The end of an open stroke.
    StrokeCap default Butt {
        Butt = "butt",
        Round = "round",
        Square = "square",
    }
);

string_enum!(
    /// The corner of a stroke.
    StrokeJoin default Miter {
        Miter = "miter",
        Round = "round",
        Bevel = "bevel",
    }
);

string_enum!(
    /// How a prototype link animates.
    TransitionKind default Instant {
        Instant = "instant",
        Dissolve = "dissolve",
        SlideLeft = "slide-left",
        SlideRight = "slide-right",
        SlideUp = "slide-up",
        SlideDown = "slide-down",
    }
);

string_enum!(
    /// Whether a file is a document or a library other documents import (`*.lib.vellum`).
    DocumentKind default Document {
        Document = "document",
        Library = "library",
    }
);

string_enum!(
    /// What a page holds: a canvas to draw on, or slides in presenting order.
    PageKind default Canvas {
        Canvas = "canvas",
        Slides = "slides",
    }
);

string_enum!(
    /// A variable's type. The words are the `type` tag of a variable in the file.
    VariableType {
        Color = "color",
        Number = "number",
        String = "string",
        Boolean = "boolean",
    }
);
