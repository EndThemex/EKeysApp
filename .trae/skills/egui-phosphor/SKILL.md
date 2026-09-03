---
name: egui-phosphor
description: Integrate and use the `egui-phosphor` crate in Rust egui/eframe applications. Use this skill when adding Phosphor icons, configuring icon fonts, building icon buttons/toolbars/navigation, choosing icon variants, or troubleshooting missing/incorrect icons in egui.
---

# egui-phosphor Skill

Use `egui-phosphor` to add Phosphor Icons to an `egui`/`eframe` application.

## 1. Scope and crate identity

Use the crate named:

```toml
egui-phosphor = "0.13"
```

Rust import path:

```rust
use egui_phosphor;
```

Do not confuse it with the separately published `egui_phosphor_icons` crate. This skill targets `egui-phosphor`.

For the current API, `egui-phosphor` provides:

- `egui_phosphor::add_to_fonts`
- `egui_phosphor::Variant`
- icon constants grouped by variant modules such as `regular`, `fill`, `bold`, `light`, and `thin`

Prefer the API exposed by the installed crate version instead of inventing helper APIs.

## 2. Installation

Add the dependency to `Cargo.toml`:

```toml
[dependencies]
egui = "0.35"
eframe = "0.35"
egui-phosphor = "0.13"
```

If the project already has compatible `egui`/`eframe` versions, preserve the project's versions unless there is a concrete compatibility problem.

Do not blindly upgrade the whole dependency tree just to add icons.

## 3. Font initialization — required

Phosphor icons are font glyphs. The font must be registered before rendering icons.

For an eframe application, initialize the font once:

```rust
fn setup_phosphor_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    egui_phosphor::add_to_fonts(
        &mut fonts,
        egui_phosphor::Variant::Regular,
    );

    ctx.set_fonts(fonts);
}
```

A typical `eframe::App::new`/startup flow can call this during initialization:

```rust
impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();

        egui_phosphor::add_to_fonts(
            &mut fonts,
            egui_phosphor::Variant::Regular,
        );

        cc.egui_ctx.set_fonts(fonts);

        Self::default()
    }
}
```

### Important

`add_to_fonts` modifies `FontDefinitions`. It is not necessary to call it every frame.

Do not repeatedly call `ctx.set_fonts(...)` from `update()` unless there is a deliberate runtime font reconfiguration.

## 4. Choosing an icon variant

The icon glyph must match the font variant that was registered.

Example:

```rust
egui_phosphor::add_to_fonts(
    &mut fonts,
    egui_phosphor::Variant::Regular,
);
```

Then use:

```rust
egui_phosphor::regular::HOUSE
egui_phosphor::regular::GEAR
egui_phosphor::regular::MAGNIFYING_GLASS
```

If using another variant, register that variant and use its corresponding module:

```rust
egui_phosphor::add_to_fonts(
    &mut fonts,
    egui_phosphor::Variant::Fill,
);

ui.label(
    egui::RichText::new(
        egui_phosphor::fill::HEART
    )
    .size(24.0)
);
```

Never mix a `regular::*` glyph with a `fill` font configuration and assume the glyph mapping will remain correct.

When multiple variants are required, inspect the installed crate's examples/API and register the required variants according to that version.

## 5. Basic rendering

Icons can be rendered anywhere egui accepts text:

```rust
ui.label(
    egui::RichText::new(egui_phosphor::regular::HOUSE)
        .size(20.0)
);
```

With color:

```rust
ui.label(
    egui::RichText::new(egui_phosphor::regular::HEART)
        .size(22.0)
        .color(egui::Color32::from_rgb(220, 70, 90))
);
```

With text:

```rust
ui.horizontal(|ui| {
    ui.label(egui_phosphor::regular::FOLDER);
    ui.label("Projects");
});
```

For icon + text layouts, prefer separate widgets when alignment and spacing matter.

## 6. Icon buttons

Use Phosphor icons for compact actions:

```rust
let icon = egui_phosphor::regular::GEAR;

if ui.button(
    egui::RichText::new(icon).size(20.0)
).clicked() {
    // Open settings
}
```

For a toolbar:

```rust
ui.horizontal(|ui| {
    if ui.button(
        egui::RichText::new(egui_phosphor::regular::ARROW_LEFT)
            .size(18.0)
    ).clicked() {
        // Back
    }

    if ui.button(
        egui::RichText::new(egui_phosphor::regular::ARROW_RIGHT)
            .size(18.0)
    ).clicked() {
        // Forward
    }

    if ui.button(
        egui::RichText::new(egui_phosphor::regular::GEAR)
            .size(18.0)
    ).clicked() {
        // Settings
    }
});
```

For destructive actions, visually distinguish the action:

```rust
ui.label(
    egui::RichText::new(egui_phosphor::regular::TRASH)
        .color(egui::Color32::RED)
);
```

Do not rely on color alone for critical actions; use clear labels/tooltips where appropriate.

## 7. Tooltips

Icon-only controls should normally have a tooltip:

```rust
let response = ui.button(
    egui::RichText::new(egui_phosphor::regular::GEAR)
        .size(18.0)
);

response.on_hover_text("Settings");

if response.clicked() {
    // ...
}
```

For dense desktop UIs, prefer icon-only controls only when the icon meaning is obvious or a tooltip is available.

## 8. Recommended icon semantics

Use semantically recognizable icons consistently:

| UI purpose | Suggested icon |
|---|---|
| Home | `HOUSE` |
| Settings | `GEAR` |
| Search | `MAGNIFYING_GLASS` |
| Add | `PLUS` |
| Delete | `TRASH` |
| Edit | `PENCIL` |
| Save | `FLOPPY_DISK` |
| Refresh | `ARROW_CLOCKWISE` |
| Back | `ARROW_LEFT` |
| Forward | `ARROW_RIGHT` |
| Close | `X` |
| Confirm | `CHECK` |
| Warning | `WARNING` |
| Information | `INFO` |
| Folder | `FOLDER` |
| File | `FILE` |
| User | `USER` |
| Menu | `LIST` |

If an icon name is uncertain, inspect the crate's generated icon modules or use the Phosphor icon catalog rather than guessing a constant name.

## 9. Icon + text composition

For navigation items:

```rust
ui.horizontal(|ui| {
    ui.label(
        egui::RichText::new(egui_phosphor::regular::HOUSE)
            .size(18.0)
    );
    ui.label("Dashboard");
});
```

For a compact status item:

```rust
ui.horizontal(|ui| {
    ui.label(
        egui::RichText::new(egui_phosphor::regular::CHECK_CIRCLE)
            .size(18.0)
            .color(egui::Color32::GREEN)
    );
    ui.label("Connected");
});
```

Keep icon size close to the surrounding text size unless the icon is intentionally emphasized.

## 10. Centralize icon usage

For a medium or large application, avoid scattering raw icon constants throughout business logic.

Create an icon module:

```rust
pub mod icons {
    pub const HOME: &str = egui_phosphor::regular::HOUSE;
    pub const SETTINGS: &str = egui_phosphor::regular::GEAR;
    pub const SEARCH: &str = egui_phosphor::regular::MAGNIFYING_GLASS;
    pub const ADD: &str = egui_phosphor::regular::PLUS;
    pub const DELETE: &str = egui_phosphor::regular::TRASH;
}
```

Then:

```rust
ui.label(
    egui::RichText::new(icons::SETTINGS)
        .size(18.0)
);
```

This makes later icon replacement and UI consistency easier.

If the application needs runtime-configurable icon names, prefer a small mapping layer rather than dynamically constructing arbitrary Unicode values.

## 11. Preserve the application's existing fonts

When integrating `egui-phosphor`, start with:

```rust
let mut fonts = egui::FontDefinitions::default();
```

Then add Phosphor fonts and call:

```rust
ctx.set_fonts(fonts);
```

Do not replace the application's entire `FontDefinitions` with a custom definition unless necessary.

The intended integration is to add Phosphor as a font/fallback while retaining normal egui text rendering.

If the application already customizes fonts, merge the Phosphor font data into the existing `FontDefinitions` instead of discarding the application's font configuration.

## 12. Common failure modes

### Icons render as boxes or missing glyphs

Check:

1. `egui-phosphor` is in `Cargo.toml`.
2. `add_to_fonts(...)` was called.
3. `ctx.set_fonts(...)` was called after modifying the definitions.
4. The icon module matches the selected `Variant`.
5. The icon constant actually exists in the installed crate version.
6. Font initialization occurs before the UI renders.

### Icon appears as an unrelated symbol

The most likely cause is a variant mismatch.

Example of a problematic combination:

```rust
egui_phosphor::add_to_fonts(
    &mut fonts,
    egui_phosphor::Variant::Fill,
);

ui.label(egui_phosphor::regular::HOUSE);
```

Use the matching variant:

```rust
ui.label(egui_phosphor::fill::HOUSE);
```

### Normal text disappears or changes unexpectedly

Check whether the application replaced `FontDefinitions` instead of extending the existing definitions.

Prefer:

```rust
let mut fonts = egui::FontDefinitions::default();
// add Phosphor
ctx.set_fonts(fonts);
```

or merge into the application's existing font definitions.

### Icons work in one screen but not another

Fonts belong to the egui context, not individual widgets. Ensure the same `egui::Context` is being used and font configuration is not being overwritten later.

## 13. Performance and binary-size considerations

Treat icon fonts as application-wide resources.

Recommended:

- Load/register fonts once.
- Reuse icon constants.
- Do not rebuild `FontDefinitions` every frame.
- Do not dynamically recreate icon font data per widget.
- Avoid loading multiple variants unless the UI actually needs them.

If binary size matters, inspect the installed `egui-phosphor` version's supported variants and choose the smallest practical configuration.

## 14. Design guidance

When building a polished egui UI:

- Use one primary icon weight/style for most controls.
- Reserve filled icons for selected, active, or emphasized states when appropriate.
- Keep icon sizes consistent within the same toolbar.
- Use spacing between icon and text instead of relying on Unicode spaces.
- Prefer familiar icons over decorative ones.
- Use tooltips for icon-only buttons.
- Do not use icons as a substitute for text when the action is ambiguous.
- Keep destructive actions visually distinct.
- Align icons using egui layout containers rather than manually inserting spaces.

## 15. Minimal complete example

```rust
use eframe::egui;

struct MyApp;

impl Default for MyApp {
    fn default() -> Self {
        Self
    }
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();

        egui_phosphor::add_to_fonts(
            &mut fonts,
            egui_phosphor::Variant::Regular,
        );

        cc.egui_ctx.set_fonts(fonts);

        Self
    }
}

impl eframe::App for MyApp {
    fn update(
        &mut self,
        ctx: &egui::Context,
        _frame: &mut eframe::Frame,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Phosphor Icons");

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        egui_phosphor::regular::HOUSE
                    )
                    .size(20.0)
                );

                ui.label("Home");

                let response = ui.button(
                    egui::RichText::new(
                        egui_phosphor::regular::GEAR
                    )
                    .size(20.0)
                );

                response.on_hover_text("Settings");
            });
        });
    }
}
```

## 16. Agent behavior

When asked to add `egui_phosphor` to an existing Rust project:

1. Inspect `Cargo.toml` and determine the existing `egui`/`eframe` versions.
2. Add a compatible `egui-phosphor` version.
3. Configure the icon font once during application initialization.
4. Use the matching icon variant module.
5. Prefer semantic icon names.
6. Use `RichText` when explicit size/color/style control is needed.
7. Add tooltips to icon-only controls.
8. Preserve existing application fonts.
9. Avoid per-frame font initialization.
10. Run `cargo check` after integration.
11. If compilation fails because of API/version differences, inspect the installed crate documentation/source and adapt to the actual version instead of assuming a newer API.

## 17. Version awareness

As of the current skill baseline, the `egui-phosphor` crate documentation shows version `0.13.0`, released in July 2026, with `egui` 0.35 compatibility.

When working in a project with a different dependency version, treat the project's `Cargo.lock` and installed crate API as authoritative.

Do not automatically migrate an existing application to the latest egui version just because the latest `egui-phosphor` release targets it.

## References

- Crate: `egui-phosphor`
- API documentation: docs.rs / `egui_phosphor`
- Icon catalog: Phosphor Icons