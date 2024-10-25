use crate::services::semantic::Semantic;
use biome_analyze::RuleSource;
use biome_analyze::{context::RuleContext, declare_lint_rule, Rule, RuleDiagnostic};
use biome_console::markup;
use biome_js_syntax::numbers::parse_js_number;
use biome_js_syntax::{
    AnyJsCallArgument, AnyJsExpression, AnyJsLiteralExpression, JsCallExpression,
};
use biome_rowan::{AstNode, AstSeparatedList};

declare_lint_rule! {
    /// Enforce the consistent use of the radix argument when using `parseInt()`.
    ///
    /// When using the `parseInt()` function it is common to omit the second argument, the radix, and let the function try to determine from the first argument what type of number it is. By default, `parseInt()` will autodetect decimal and hexadecimal (via `0x` prefix). Prior to ECMAScript 5, `parseInt()` also autodetected octal literals, which caused problems because many developers assumed a leading `0` would be ignored.
    ///
    /// This confusion led to the suggestion that you always use the radix parameter to `parseInt()` to eliminate unintended consequences.
    ///
    /// ## Examples
    ///
    /// ### Invalid
    ///
    /// ```js,expect_diagnostic
    /// parseInt("071");
    /// parseInt(someValue);
    /// parseInt("071", "abc");
    /// parseInt("071", 37);
    /// parseInt();
    /// ```
    ///
    /// ### Valid
    ///
    /// ```js
    /// parseInt("071", 10);
    /// parseInt("071", 8);
    /// parseFloat(someValue);
    /// ```
    ///
    pub UseParseIntRadix {
        version: "next",
        name: "useParseIntRadix",
        language: "js",
        recommended: true,
        sources: &[RuleSource::Eslint("radix")],
    }
}

impl Rule for UseParseIntRadix {
    type Query = Semantic<JsCallExpression>;
    type State = State;
    type Signals = Option<Self::State>;
    type Options = ();

    fn run(ctx: &RuleContext<Self>) -> Self::Signals {
        let call_expression = ctx.query();

        let object_name = call_expression.callee().ok()?.get_callee_object_name()?;

        if !matches!(object_name.text(), "Number" | "parseInt") {
            return None;
        }

        let member_name = call_expression.callee().ok()?.get_callee_member_name()?;
        if member_name.text() != "parseInt" {
            return None;
        }

        let mut arguments = call_expression.arguments().ok()?.args().iter();

        let Some(first_argument) = arguments.next() else {
            return Some(State::MissingParameters);
        };
        let first_argument = first_argument.ok()?;

        // If the first argument is a spread we can't check the arguments, so we skip
        if first_argument.as_js_spread().is_some() {
            return None;
        }

        let Some(second_argument) = arguments.next() else {
            return Some(State::MissingRadix);
        };
        let second_argument = second_argument.ok()?;

        let AnyJsCallArgument::AnyJsExpression(radix_argument) = second_argument else {
            // Ignore spread argument
            return None;
        };

        if !is_valid_radix(&radix_argument)? {
            return Some(State::InvalidRadix);
        }

        None
    }

    fn diagnostic(ctx: &RuleContext<Self>, state: &Self::State) -> Option<RuleDiagnostic> {
        let node = ctx.query();

        let (title, note) = match state {
            State::MissingParameters => (
                markup!("Missing parameters"),
                markup!("Will always return "<Emphasis>"NaN"</Emphasis>),
            ),
            State::MissingRadix => (
                markup!("Missing radix parameter"),
                markup!("Add a non-fractional number between 2 and 36"),
            ),
            State::InvalidRadix => (
                markup!("Invalid radix parameter"),
                markup!("Radix must be a non-fractional number between 2 and 36"),
            ),
        };

        Some(RuleDiagnostic::new(rule_category!(), node.range(), title).note(note))
    }
}

pub enum State {
    MissingParameters,
    MissingRadix,
    InvalidRadix,
}

/// Checks whether a given node is a valid value of radix or not.
///
/// The following values are invalid:
/// - A literal except integers between 2 and 36
/// - `undefined`
fn is_valid_radix(argument: &AnyJsExpression) -> Option<bool> {
    Some(match argument {
        AnyJsExpression::AnyJsLiteralExpression(any_js_literal_expression) => {
            is_valid_radix_value(any_js_literal_expression)?
        }
        AnyJsExpression::JsIdentifierExpression(js_identifier_expression) => {
            js_identifier_expression
                .name()
                .ok()?
                .value_token()
                .ok()?
                .text()
                != "undefined"
        }
        _ => true,
    })
}

/// Checks if a literal is an integer between 2 and 36
fn is_valid_radix_value(literal: &AnyJsLiteralExpression) -> Option<bool> {
    let AnyJsLiteralExpression::JsNumberLiteralExpression(js_number_literal_expression) = literal
    else {
        return Some(false);
    };

    let value_token = js_number_literal_expression.value_token().ok()?;
    let number = parse_js_number(value_token.text_trimmed())?;

    let Some(number) = f64_to_i64(number) else {
        return Some(false);
    };

    Some((2..=36).contains(&number))
}

/// Convert an f64 to on i64 only if it is not fractional
/// Requires the f64 to be finite and not NaN
fn f64_to_i64(value: f64) -> Option<i64> {
    if value.fract() != 0.0 || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return None;
    }

    Some(value as i64)
}
