# Component Readiness Matrix

**Audited:** 2026-08-14

This matrix records repository evidence, not a stable-API promise. `Model`
means deterministic state/model tests exist, `Interaction` means GPUI pointer
or keyboard tests exist, and `Partial` means semantics are present but the
complete platform accessibility behavior has not been physically audited.

| Surface | Documentation | Automated evidence | Keyboard | Accessibility | Status |
| --- | --- | --- | --- | --- | --- |
| Accordion | Yes | Interaction | Enter/Space activation | Button/expanded | Preview |
| Alert | Yes | Gallery | N/A | Partial | Preview |
| AutoComplete | Yes | Model | Text input + directional option navigation | Listbox/options | Preview |
| Avatar | Yes | Model | N/A | Partial | Preview |
| Badge | Yes | Gallery | N/A | Decorative | Preview |
| Breadcrumb | Yes | Interaction | Enter/Space activation | Link | Preview |
| Button | Yes | Interaction | Native activation | Button/disabled | Preview |
| Card | Yes | Gallery | N/A | Grouping only | Preview |
| CascadeSelect | Yes | Model | Trigger/option activation | Button/options | Preview |
| Checkbox | Yes | Interaction | Space/Enter | Checkbox/checked | Preview |
| Chip | Yes | Model | Enter/Space activation | Button/selection | Preview |
| ColorPicker | Yes | Model | Directional swatch navigation/activation | Radio/selection | Preview |
| CommandPalette | Yes | Interaction | Full navigation/dismissal | Dialog/listbox/options | Preview |
| ConfirmDialog | Yes | Interaction | Button activation/dismissal | Alert dialog/buttons | Preview |
| ConfirmPopup | Yes | Model | Button activation | Dialog/buttons | Preview |
| DataTable | Yes | Model + interaction + stress | Full row navigation/selection | Table/row/cell | Preview |
| DataView | Yes | Model | Child button activation | Child buttons | Preview |
| DatePicker | Yes | Interaction | Trigger + calendar grid navigation | Button/expanded/selection | Preview |
| Dialog | Yes | Interaction | Button activation/dismissal | Dialog | Preview |
| Dock | Yes | Model + interaction + layout stress | Workspace commands | Partial | Preview |
| Drawer | Yes | Interaction | Button activation/dismissal | Dialog/close button | Preview |
| Fieldset | Yes | Model | N/A | Grouping only | Preview |
| FilePicker | Yes | Model | Button activation | Button via child | Preview |
| Form / FormField / FormSummary | Yes | Model | Child controls | Partial | Preview |
| IconButton | Yes | Interaction via button suite | Native activation | Button/label/disabled | Preview |
| Image | Yes | Model + layout interaction | N/A | Image/label | Preview |
| InputNumber | Yes | Model + interaction | Arrows/Home/End | Spin button/range | Preview |
| InputOtp | Yes | Model | Text entry/backspace | Partial | Preview |
| Label | Yes | Gallery | N/A | Text | Preview |
| Listbox | Yes | Interaction | Directional navigation + activation | Listbox/options | Preview |
| Markdown / HtmlFragment | Yes | Parser/model | Link host behavior | Partial | Preview |
| Menu / Menubar / TieredMenu / ContextMenu / PanelMenu | Yes | Model + interaction | Navigation/typeahead/activation/dismissal | Menu/menu item | Preview |
| Message | Yes | Gallery | N/A | Partial | Preview |
| MetricCard | Yes | Model | N/A | Grouping only | Preview |
| MultiSelect | Yes | Interaction | Directional navigation + activation | Button/listbox/options | Preview |
| Paginator | Yes | Model + interaction | All page activation | Button/selection | Preview |
| Panel | Yes | Interaction | Header activation | Button/expanded | Preview |
| PickList | Yes | Model | Directional option transfer | Listbox/options | Preview |
| Popover | Yes | Interaction | Trigger/dismissal host path | Partial | Preview |
| Progress | Yes | Model | N/A | Progress/range | Preview |
| PropertyList | Yes | Model | N/A | Grouping only | Preview |
| Radio | Yes | Interaction | Space/Enter | Radio/checked | Preview |
| ScrollArea | Yes | Gallery | Platform scrolling | Partial | Preview |
| SearchInput / PasswordInput / TextArea / TextInput | Yes | Interaction | Native editing/clipboard/IME hooks | Text input | Preview |
| Select | Yes | Model + interaction | Full navigation/typeahead/dismissal | Button/listbox/options | Preview |
| Separator | Yes | Gallery | N/A | Decorative | Preview |
| Slider | Yes | Model + interaction | Arrows/Home/End | Slider/range | Preview |
| Spinner | Yes | Gallery | N/A | Partial | Preview |
| Splitter | Yes | Model | Layout only | Grouping only | Preview |
| Stepper | Yes | Model + narrow-layout policy | N/A | Partial | Preview |
| Switch | Yes | Interaction | Space/Enter | Switch/checked | Preview |
| TabMenu | Yes | Interaction | Directional navigation/activation | Tab/selected | Preview |
| Tabs | Yes | Model + interaction | Arrows/Home/End/activation | Tab/selected | Preview |
| Tag | Yes | Gallery | Remove button host path | Partial | Preview |
| Timeline | Yes | Model | N/A | Partial | Preview |
| Toast / ToastStack | Yes | Interaction | Close button | Alert/status/close button | Preview |
| Toolbar | Yes | Gallery | Child controls | Grouping only | Preview |
| Tooltip | Yes | Gallery | Host trigger behavior | Tooltip | Preview |
| TreeSelect | Yes | Model | Directional trigger/item activation | Button/tree/tree item | Preview |
| TreeTable | Yes | Model | Row selection + branch toggle | Table/row/button | Preview |
| TreeView | Yes | Model + interaction + stress | Full tree navigation/selection | Tree/tree item | Preview |
| VirtualList | Yes | Model + stress | Host content | N/A | Preview |
| Charts | Yes | Model + interaction geometry + stress + benchmarks | Point navigation/zoom/reset | Pointer-following datum tooltip + bounded summary + controls | Preview |
| CodeEditor | Yes | Model + large-file benchmarks | Editing/page navigation/clipboard/undo routing | Partial | Experimental |
| Terminal | Yes | Model + stress + benchmarks + ConPTY | Terminal input/selection | Partial | Preview |

## Stable-release gate

No row graduates beyond `Preview` until the relevant macOS, Windows, and Linux
cells in `docs/platform-smoke.md` are recorded. Rows marked `Host/pointer` or
`Partial` require additional built-in keyboard behavior or an explicit host
contract and physical accessibility verification before stable support.
