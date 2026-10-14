//! The examples shown in README.md, so the documentation can't drift from real output.

use spotcalc::calc::Calculator;
use spotcalc::rates::{Rates, SharedRates};

fn assert_eval(calc: &mut Calculator, input: &str, expected: &str) {
    assert_eq!(
        calc.evaluate(input).as_deref(),
        Ok(expected),
        "input: {input}"
    );
}

#[test]
fn readme_examples() {
    let mut calc = Calculator::new();
    assert_eval(&mut calc, "15% of 80", "12");
    assert_eval(&mut calc, "3 ft to cm", "91.44 cm");
    assert_eval(&mut calc, "0xff to binary", "11111111");
    assert_eval(&mut calc, "x = 4", "4");
    assert_eval(&mut calc, "x^2 + ans", "20");
    assert_eval(&mut calc, "rate = 0.05", "0.05");
    assert_eval(&mut calc, "1200 * rate", "60");
}

#[test]
fn readme_currency_example() {
    // Fixed sample rate; real rates change daily.
    let json = r#"{"date":"2026-10-13","usd":{"usd":1,"twd":32.1318}}"#;
    let rates = SharedRates::with(Rates::parse(json).expect("sample"));
    let mut calc = Calculator::with_rates(rates);
    assert_eval(&mut calc, "100 USD to TWD", "3213.18 TWD");
}
