use clak_engine::engine;
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_single_char(c: &mut Criterion) {
    c.bench_function("telex_single_a", |b| {
        b.iter(|| engine::convert_telex(black_box("a"), true, false))
    });
    c.bench_function("telex_single_as_accent", |b| {
        b.iter(|| engine::convert_telex(black_box("as"), true, false))
    });
}

fn bench_short_words(c: &mut Criterion) {
    c.bench_function("telex_tooi", |b| {
        b.iter(|| engine::convert_telex(black_box("tooi"), true, false))
    });
    c.bench_function("telex_tieengs", |b| {
        b.iter(|| engine::convert_telex(black_box("tieengs"), true, false))
    });
    c.bench_function("telex_nguwowif", |b| {
        b.iter(|| engine::convert_telex(black_box("nguwowif"), true, false))
    });
    c.bench_function("telex_dduwowcj", |b| {
        b.iter(|| engine::convert_telex(black_box("dduwowcj"), true, false))
    });
}

fn bench_long_text(c: &mut Criterion) {
    let sentence = "tooi ddeens tuwf vuofn cuar mej tooi, maf toi laf con trai duy nhaats";
    c.bench_function("telex_sentence", |b| {
        b.iter(|| engine::convert_telex(black_box(sentence), true, false))
    });
}

fn bench_incremental_typing(c: &mut Criterion) {
    let typing_sequences: &[&[&str]] = &[
        &[
            "x",
            "xi",
            "xin",
            "xin ",
            "xin c",
            "xin ch",
            "xin cha",
            "xin chaf",
            "xin chafo",
        ],
        &[
            "t",
            "ti",
            "tie",
            "tiee",
            "tieen",
            "tieeng",
            "tieengs",
            "tieengs ",
            "tieengs V",
            "tieengs Vi",
            "tieengs Vie",
            "tieengs Viee",
            "tieengs Viej",
            "tieengs Vieejt",
        ],
        &[
            "d", "dd", "ddu", "dduw", "dduwo", "dduwow", "dduwowc", "dduwowcj",
        ],
        &[
            "n", "ng", "ngu", "nguw", "nguwo", "nguwow", "nguwowi", "nguwowif",
        ],
        &["t", "to", "tof", "tofa", "tofan"],
    ];

    let mut group = c.benchmark_group("incremental_typing");
    for (seq_idx, seq) in typing_sequences.iter().enumerate() {
        group.bench_function(format!("seq_{}", seq_idx), |b| {
            b.iter(|| {
                for input in *seq {
                    let _ = engine::convert_telex(black_box(input), false, true);
                }
            })
        });
    }
    group.finish();
}

fn bench_rapid_accumulation(c: &mut Criterion) {
    let chars: Vec<String> = "tieengs vieejt dduwowcj nguwowif"
        .chars()
        .scan(String::new(), |acc, ch| {
            acc.push(ch);
            Some(acc.clone())
        })
        .collect();

    c.bench_function("rapid_50_keystrokes", |b| {
        b.iter(|| {
            for s in &chars {
                let _ = engine::convert_telex(black_box(s.as_str()), false, true);
            }
        })
    });
}

fn bench_vni(c: &mut Criterion) {
    c.bench_function("vni_tie61ng", |b| {
        b.iter(|| engine::convert_vni(black_box("tie61ng"), false, true))
    });
    c.bench_function("vni_vie65t", |b| {
        b.iter(|| engine::convert_vni(black_box("vie65t"), false, true))
    });
}

fn bench_teipvni(c: &mut Criterion) {
    c.bench_function("teipvni_vie61t", |b| {
        b.iter(|| engine::convert_teip_vni(black_box("vie61t"), false, true))
    });
    c.bench_function("teipvni_tie61ng", |b| {
        b.iter(|| engine::convert_teip_vni(black_box("tie61ng"), false, true))
    });
}

fn bench_charset(c: &mut Criterion) {
    let text = "tiếng Việt là ngôn ngữ của người Việt Nam, được viết bằng chữ Latinh với các dấu thanh điệu";
    c.bench_function("charset_encode_tcvn3", |b| {
        b.iter(|| {
            clak_engine::charset::encode(black_box(text), clak_engine::charset::VietCharset::TCVN3)
        })
    });
    c.bench_function("charset_decode_tcvn3", |b| {
        let encoded = clak_engine::charset::encode(text, clak_engine::charset::VietCharset::TCVN3);
        b.iter(|| {
            clak_engine::charset::decode(
                black_box(&encoded),
                clak_engine::charset::VietCharset::TCVN3,
            )
        })
    });
    c.bench_function("charset_encode_vniwin", |b| {
        b.iter(|| {
            clak_engine::charset::encode(black_box(text), clak_engine::charset::VietCharset::VNIWin)
        })
    });
    c.bench_function("charset_decode_vniwin", |b| {
        let encoded = clak_engine::charset::encode(text, clak_engine::charset::VietCharset::VNIWin);
        b.iter(|| {
            clak_engine::charset::decode(
                black_box(&encoded),
                clak_engine::charset::VietCharset::VNIWin,
            )
        })
    });
    c.bench_function("charset_remove_tone", |b| {
        b.iter(|| clak_engine::charset::remove_tone(black_box(text)))
    });
}

fn bench_surrounding_text(c: &mut Criterion) {
    let base_paragraph = "Đây là văn bản tiếng Việt mẫu dùng để kiểm thử hiệu năng bộ gõ khi có surrounding text từ ứng dụng. ";
    let text_1k = base_paragraph.repeat(10);
    let chars_1k_count = text_1k.chars().count();
    let text_10k = base_paragraph.repeat(100);
    let chars_10k_count = text_10k.chars().count();

    let mut group = c.benchmark_group("surrounding_text");
    group.bench_function("keystroke_no_surrounding", |b| {
        let mut ctx = clak_engine::ClakContext::new(clak_engine::Method::Telex);
        b.iter(|| ctx.process_key(black_box(0x61), black_box("a"), false, None, 0, 0))
    });

    group.bench_function("keystroke_surrounding_1k", |b| {
        let mut ctx = clak_engine::ClakContext::new(clak_engine::Method::Telex);
        b.iter(|| {
            ctx.process_key(
                black_box(0x61),
                black_box("a"),
                false,
                Some(black_box(&text_1k)),
                black_box(chars_1k_count),
                black_box(chars_1k_count),
            )
        })
    });

    group.bench_function("keystroke_surrounding_10k", |b| {
        let mut ctx = clak_engine::ClakContext::new(clak_engine::Method::Telex);
        b.iter(|| {
            ctx.process_key(
                black_box(0x61),
                black_box("a"),
                false,
                Some(black_box(&text_10k)),
                black_box(chars_10k_count),
                black_box(chars_10k_count),
            )
        })
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_single_char,
    bench_short_words,
    bench_long_text,
    bench_incremental_typing,
    bench_rapid_accumulation,
    bench_vni,
    bench_teipvni,
    bench_charset,
    bench_surrounding_text,
);
criterion_main!(benches);
