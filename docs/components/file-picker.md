# FilePicker

Render a controlled file-import workflow surface.

```rust
use gpui::PathPromptOptions;
use guic::prelude::FilePicker;

FilePicker::new("attachments")
    .files(vec!["design.pdf", "notes.md"])
    .label("Attach files")
    .on_request_pick(|_, cx| {
        let selected = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Attach".into()),
        });
        // Spawn a host task that awaits `selected`, then update controlled state.
    })
```

The application opens the native dialog and passes selected file labels through
`FilePicker::files`. GPUI's `App::prompt_for_paths` and
`App::prompt_for_new_path` provide native open/save dialogs. Handle cancellation
and platform errors in the application.
