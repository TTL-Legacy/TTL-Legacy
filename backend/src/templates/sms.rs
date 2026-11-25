use crate::templates::{NotificationTemplate, TemplateContext};

/// Builds the SMS body for a notification.
pub fn build_sms_body(template: &NotificationTemplate, context: &TemplateContext) -> String {
    let mut body = template.body.clone();

    for (key, value) in context.variables() {
        let placeholder = format!("{{{{{}}}}}", key);
        body = body.replace(&placeholder, value);
    }

    body
}

/// Builds the SMS subject (used as a short title/preview) for a notification.
pub fn build_sms_subject(template: &NotificationTemplate, context: &TemplateContext) -> String {
    let mut subject = template.subject.clone();

    for (key, value) in context.variables() {
        let placeholder = format!("{{{{{}}}}}", key);
        subject = subject.replace(&placeholder, value);
    }

    subject
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::{NotificationTemplate, TemplateContext};

    fn sample_template() -> NotificationTemplate {
        NotificationTemplate {
            subject: "Hello {{name}}".to_string(),
            body: "Your code is {{code}}".to_string(),
        }
    }

    fn sample_context() -> TemplateContext {
        let mut context = TemplateContext::new();
        context.insert("name", "Alice");
        context.insert("code", "123456");
        context
    }

    #[test]
    fn builds_sms_body_with_substitutions() {
        let body = build_sms_body(&sample_template(), &sample_context());
        assert_eq!(body, "Your code is 123456");
    }

    #[test]
    fn builds_sms_subject_with_substitutions() {
        let subject = build_sms_subject(&sample_template(), &sample_context());
        assert_eq!(subject, "Hello Alice");
    }

    #[test]
    fn leaves_unknown_placeholders_untouched() {
        let template = NotificationTemplate {
            subject: "Hi {{unknown}}".to_string(),
            body: "Code {{code}} for {{unknown}}".to_string(),
        };
        let context = sample_context();

        assert_eq!(build_sms_subject(&template, &context), "Hi {{unknown}}");
        assert_eq!(
            build_sms_body(&template, &context),
            "Code 123456 for {{unknown}}"
        );
    }
}