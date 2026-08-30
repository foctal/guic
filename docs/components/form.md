# Form

`Form`, `FormField`, and `FormSummary` provide consistent layout and validation
presentation while leaving values and submission policy under application
control.

```rust,ignore
use guic_components::{
    Button, Form, FormField, FormSummary, TextInput, ValidationIssue,
    ValidationSeverity,
};

Form::new("account")
    .label("Account settings")
    .child(FormSummary::new(
        "account-errors",
        vec![ValidationIssue::error(
            "email",
            "Email",
            "Enter a valid email address",
        )],
    ))
    .child(
        FormField::new("email", "Email", email_input)
            .required(true)
            .description("Used for account notifications")
            .validation(
                ValidationSeverity::Error,
                "Enter a valid email address",
            ),
    )
    .action(Button::new("save", "Save"));
```

The host owns validation timing, focus movement, and submission. Use stable
field identifiers so an application can move focus from a summary issue to its
control. Required state is included in the accessible field label, and disabled
and validation metadata are attached to the field group.
