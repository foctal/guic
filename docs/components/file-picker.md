# FilePicker

## Purpose

Render a controlled file-import workflow surface.

## Import

```rust
use guic::prelude::FilePicker;
```

## Basic Usage

```rust,ignore
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

## Notes

`FilePicker` deliberately does not launch native dialogs itself. The host owns
the platform integration, then passes selected file labels back through
`FilePicker::files`. GPUI's `App::prompt_for_paths` and
`App::prompt_for_new_path` provide the native cross-platform open/save dialog
boundary; applications should handle cancellation and Linux launch errors
explicitly.
