# Components

Import components from `guic::prelude`. Most are controlled: the application
supplies values and applies changes received through callbacks. Stateful inputs
such as `TextInput` are retained as GPUI entities.

The following components are available with the default `components` feature.
Examples show view-construction fragments; names such as `cx`, `nodes`, and
`is_open` refer to application context or state.

## Text and status

[Label](components/label.md),
[Badge](components/badge.md),
[Tag](components/tag.md),
[Chip](components/chip.md),
[Alert](components/alert.md),
[Message](components/message.md),
[Toast](components/toast.md),
[Progress](components/progress.md),
[Spinner](components/spinner.md),
[Skeleton](components/skeleton.md),
[Avatar](components/avatar.md),
[Image](components/image.md),
[Property List](components/property-list.md),
[Metric Card](components/metric-card.md).

## Input and forms

[Button](components/button.md),
[IconButton](components/icon-button.md),
[Checkbox](components/checkbox.md),
[Radio](components/radio.md),
[Switch](components/switch.md),
[TextInput](components/text-input.md),
[SearchInput](components/search-input.md),
[PasswordInput](components/password-input.md),
[TextArea](components/text-area.md),
[InputNumber](components/input-number.md),
[InputOtp](components/input-otp.md),
[Slider](components/slider.md),
[Select](components/select.md),
[MultiSelect](components/multi-select.md),
[AutoComplete](components/auto-complete.md),
[DatePicker](components/date-picker.md),
[ColorPicker](components/color-picker.md),
[CascadeSelect](components/cascade-select.md),
[TreeSelect](components/tree-select.md),
[FilePicker](components/file-picker.md),
[InputGroup](components/input-group.md),
[Form](components/form.md),
[Fieldset](components/fieldset.md).

## Navigation and collections

[Tabs](components/tabs.md),
[TabMenu](components/tab-menu.md),
[Breadcrumb](components/breadcrumb.md),
[Stepper](components/stepper.md),
[Menu](components/menu.md),
[Menubar](components/menubar.md),
[TieredMenu](components/tiered-menu.md),
[ContextMenu](components/context-menu.md),
[PanelMenu](components/panel-menu.md),
[Listbox](components/listbox.md),
[PickList](components/pick-list.md),
[Paginator](components/paginator.md),
[DataView](components/data-view.md),
[TreeTable](components/tree-table.md),
[Timeline](components/timeline.md).

## Layout and overlays

[Card](components/card.md),
[Panel](components/panel.md),
[Accordion](components/accordion.md),
[Toolbar](components/toolbar.md),
[Separator](components/separator.md),
[ScrollArea](components/scroll-area.md),
[VirtualList](components/virtual-list.md),
[Splitter](components/splitter.md),
[Dialog](components/dialog.md),
[ConfirmDialog](components/confirm-dialog.md),
[ConfirmPopup](components/confirm-popup.md),
[Popover](components/popover.md),
[Drawer](components/drawer.md),
[Tooltip](components/tooltip.md),
[Command Palette](components/command-palette.md).

## Optional features

| Component | Feature on `guic` | Implementation |
| --- | --- | --- |
| [DataTable](components/data-table.md) | `data-table` | `guic-components` |
| [TreeView](components/tree-view.md) | `tree` | `guic-components` |
| [Dock](components/dock.md) | `dock` | `guic-components` |
| [Markdown / HtmlFragment](components/markdown.md) | `markdown` | `guic-components` |
| [Charts](components/charts.md) | `charts` | `guic-charts` |
| [CodeEditor](components/code-editor.md) | `editor` | `guic-editor` |

For example, enable `tree` and `dock` in your `guic` dependency's `features`
list. Direct `guic-components` dependencies use the same feature names.

See [shared component behavior](component-behavior.md) for sizing, identity, and
focus, and [API stability](api-stability.md) for support status. Run the gallery
from the repository root:

```sh
cargo run -p guic-sample-component-gallery
```
