# InputGroup

`InputGroup` composes existing native inputs, text addons and buttons into one
horizontal row. It introduces no new input state, event interception or tab stop.
`InputGroupAddon` displays a non-interactive prefix or suffix with control-sized
height and typography.

```rust,no_run
use gpui::{IntoElement, ParentElement as _, Styled as _, div};
use guic::prelude::*;

fn search_group(input: Entity<TextInput>) -> impl IntoElement {
    InputGroup::new("search-group")
        .label("Search documents")
        .child(InputGroupAddon::new("Find"))
        .child(div().flex_1().min_w_0().child(input))
        .child(Button::new("Search"))
}
```

Create and retain the input entity in the host view, and register the action
handler on the button. Child controls keep their own visual borders and accessible
labels. The group's label describes the relationship between controls; it does
not replace each input's label. Match addon and control sizes with `.size(...)`.
Use a flexible wrapper as above to let the input shrink in narrow containers.
