use criterion::{criterion_group, criterion_main, Criterion};

fn benchmark_pricing(c: &mut Criterion) {
    use shuji_app_lib::pricing::PricingConfig;

    let config = PricingConfig::default_v4();
    c.bench_function("pricing_estimate_cost", |b| {
        b.iter(|| {
            let _ = config.estimate_cost("deepseek-v4-flash", 1000, 200, 500, "usd");
        });
    });

    c.bench_function("pricing_find_entry", |b| {
        b.iter(|| {
            let _ = config.find_entry("deepseek-v4-flash");
        });
    });
}

criterion_group!(benches, benchmark_pricing);
criterion_main!(benches);
