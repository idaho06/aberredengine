use super::*;
use aberred_core::components::shape::Shape;
use aberred_core::math::{Color, Vec2};

/// Runs `f` on the shape set by an earlier `with_shape_*` kind call, or raises the
/// chaining error for `method`.
fn modify_shape(
    this: &mut LuaEntityBuilder,
    method: &str,
    f: impl FnOnce(Shape) -> Shape,
) -> LuaResult<()> {
    let Some(shape) = this.cmd.shape.take() else {
        return Err(LuaError::runtime(format!(
            "{method}() requires a with_shape_*() first"
        )));
    };
    this.cmd.shape = Some(f(shape));
    Ok(())
}

pub(super) fn register<M: LuaUserDataMethods<LuaEntityBuilder>>(
    methods: &mut M,
    meta: &mut Option<Vec<BuilderMethodDef>>,
) {
    builder_method!(
        methods,
        meta,
        "with_shape_point",
        "Draw a single pixel (needs a position and a z index to render)",
        [
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (r, g, b, a): (u8, u8, u8, u8)| {
            this.cmd.shape = Some(Shape::point(Color::new(r, g, b, a)));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_line",
        "Draw a 1 px line from the local origin to (to_x, to_y)",
        [
            ("to_x", "number"),
            ("to_y", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (to_x, to_y, r, g, b, a): (f32, f32, u8, u8, u8, u8)| {
            this.cmd.shape = Some(Shape::line(Vec2::new(to_x, to_y), Color::new(r, g, b, a)));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_rect",
        "Draw a rectangle (outline; add with_shape_filled() to fill) with its top-left corner at the local origin",
        [
            ("width", "number"),
            ("height", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (w, h, r, g, b, a): (f32, f32, u8, u8, u8, u8)| {
            this.cmd.shape = Some(Shape::rect(Vec2::new(w, h), Color::new(r, g, b, a)));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_circle",
        "Draw a circle (outline; add with_shape_filled() to fill) centred on the local origin",
        [
            ("radius", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (radius, r, g, b, a): (f32, u8, u8, u8, u8)| {
            this.cmd.shape = Some(Shape::circle(radius, Color::new(r, g, b, a)));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_ellipse",
        "Draw an axis-aligned ellipse (outline; add with_shape_filled() to fill); it ignores rotation",
        [
            ("radius_x", "number"),
            ("radius_y", "number"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (rx, ry, r, g, b, a): (f32, f32, u8, u8, u8, u8)| {
            this.cmd.shape = Some(Shape::ellipse(Vec2::new(rx, ry), Color::new(r, g, b, a)));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_polygon",
        "Draw a polygon through local-space points (outline; add with_shape_filled() to fill); triangulated once here",
        [
            ("points", "table"),
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (points, r, g, b, a): (LuaTable, u8, u8, u8, u8)| {
            let points = parse_points(&points)?;
            this.cmd.shape = Some(Shape::polygon(&points, Color::new(r, g, b, a)));
            Ok(())
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_filled",
        "Fill the shape's interior with its color",
        [],
        |_, this: &mut LuaEntityBuilder, (): ()| {
            modify_shape(this, "with_shape_filled", Shape::filled)
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_outline_color",
        "Draw a 1 px outline in this color over a filled shape",
        [
            ("r", "integer"),
            ("g", "integer"),
            ("b", "integer"),
            ("a", "integer")
        ],
        |_, this: &mut LuaEntityBuilder, (r, g, b, a): (u8, u8, u8, u8)| {
            modify_shape(this, "with_shape_outline_color", |shape| {
                shape.with_outline_color(Color::new(r, g, b, a))
            })
        }
    );

    builder_method!(
        methods,
        meta,
        "with_shape_origin",
        "Set the shape's local pivot, the point that sits on the entity position and is the center of scale and rotation",
        [("x", "number"), ("y", "number")],
        |_, this: &mut LuaEntityBuilder, (x, y): (f32, f32)| {
            modify_shape(this, "with_shape_origin", |shape| {
                shape.with_origin(Vec2::new(x, y))
            })
        }
    );
}

#[cfg(test)]
mod tests {
    use super::super::test_helpers::{assert_runtime_error, built_spawn_cmd};
    use aberred_core::components::shape::Shape;
    use aberred_core::math::{Color, Vec2};

    const C: Color = Color {
        r: 1,
        g: 2,
        b: 3,
        a: 4,
    };

    fn shape_of(chain: &str) -> Shape {
        built_spawn_cmd(&format!("engine.spawn(){chain}:build()"))
            .shape
            .expect("the chain sets a shape")
    }

    #[test]
    fn each_kind_stores_the_matching_shape() {
        assert_eq!(shape_of(":with_shape_point(1, 2, 3, 4)"), Shape::point(C));
        assert_eq!(
            shape_of(":with_shape_line(5, 6, 1, 2, 3, 4)"),
            Shape::line(Vec2::new(5.0, 6.0), C)
        );
        assert_eq!(
            shape_of(":with_shape_rect(7, 8, 1, 2, 3, 4)"),
            Shape::rect(Vec2::new(7.0, 8.0), C)
        );
        assert_eq!(
            shape_of(":with_shape_circle(9, 1, 2, 3, 4)"),
            Shape::circle(9.0, C)
        );
        assert_eq!(
            shape_of(":with_shape_ellipse(3, 4, 1, 2, 3, 4)"),
            Shape::ellipse(Vec2::new(3.0, 4.0), C)
        );
        assert_eq!(
            shape_of(":with_shape_polygon({ {x=0,y=0}, {x=4,y=0}, {x=2,y=3} }, 1, 2, 3, 4)"),
            Shape::polygon(&[Vec2::ZERO, Vec2::new(4.0, 0.0), Vec2::new(2.0, 3.0)], C)
        );
    }

    #[test]
    fn a_later_kind_replaces_the_earlier_shape_entirely() {
        let shape = shape_of(
            ":with_shape_rect(2, 2, 9, 9, 9, 9):with_shape_filled():with_shape_origin(1, 1)\
             :with_shape_circle(5, 1, 2, 3, 4)",
        );
        assert_eq!(shape, Shape::circle(5.0, C));
    }

    #[test]
    fn modifiers_set_fill_outline_and_origin() {
        let shape = shape_of(
            ":with_shape_rect(2, 2, 1, 2, 3, 4):with_shape_filled()\
             :with_shape_outline_color(5, 6, 7, 8):with_shape_origin(1, 1.5)",
        );
        assert_eq!(
            shape,
            Shape::rect(Vec2::new(2.0, 2.0), C)
                .filled()
                .with_outline_color(Color::new(5, 6, 7, 8))
                .with_origin(Vec2::new(1.0, 1.5))
        );
    }

    #[test]
    fn modifiers_require_a_shape_kind_first() {
        for (call, method) in [
            ("with_shape_filled()", "with_shape_filled"),
            (
                "with_shape_outline_color(1, 2, 3, 4)",
                "with_shape_outline_color",
            ),
            ("with_shape_origin(1, 2)", "with_shape_origin"),
        ] {
            assert_runtime_error(
                &format!("engine.spawn():{call}"),
                &format!("{method}() requires a with_shape_*() first"),
            );
        }
    }

    #[test]
    fn polygon_points_must_be_tables_with_numeric_x_and_y() {
        for points in [
            "{ {x=0,y=0}, 5 }",
            "{ {x=0,y=0}, {x=1} }",
            "{ {x=0,y=0}, {x='a',y=1} }",
        ] {
            assert_runtime_error(
                &format!("engine.spawn():with_shape_polygon({points}, 1, 2, 3, 4)"),
                "point 2",
            );
        }
    }

    #[test]
    fn colour_channels_out_of_u8_range_are_rejected() {
        assert_runtime_error(
            "engine.spawn():with_shape_rect(1, 1, 256, 0, 0, 255)",
            "with_shape_rect",
        );
    }
}
