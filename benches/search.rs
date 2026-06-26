use fractional_cascading::FCSearcher;

use rand::SeedableRng;

use criterion::BenchmarkId;
use criterion::Criterion;
use criterion::{criterion_group, criterion_main};
use rand::seq::IteratorRandom;

const KB: u64 = 1024;

const RNG_SEED: u64 = 42;
const NUM_CATALOGS: usize = 40;
const CATALOG_SIZES: &[u64] = &[KB, 2 * KB, 4 * KB, 8 * KB, 16 * KB, 32 * KB];
const NUM_KEYS: usize = 50;

fn search_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");

    let mut rng = rand::rngs::StdRng::seed_from_u64(RNG_SEED);

    for &size in CATALOG_SIZES {
        let catalogs: Vec<Vec<_>> = (0..NUM_CATALOGS).map(|_| (0..size).collect()).collect();
        let searcher = FCSearcher::new(&catalogs);

        let keys = (0..size).choose_multiple(&mut rng, NUM_KEYS);

        group.bench_with_input(
            BenchmarkId::new("BinarySearch", size),
            &catalogs,
            |b, catalogs| {
                b.iter(|| {
                    for key in &keys {
                        let _ = catalogs
                            .iter()
                            .map(|catalog| catalog.partition_point(|x| x < key))
                            .collect::<Vec<_>>();
                    }
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("FCSearcher", size),
            &searcher,
            |b, searcher| {
                b.iter(|| {
                    for key in &keys {
                        let _ = searcher.search(key);
                    }
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, search_benchmark);
criterion_main!(benches);
