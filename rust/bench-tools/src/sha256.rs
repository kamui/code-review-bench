//! SHA-256 as `hashlib.sha256(data).hexdigest()` spells it.

use sha2::{Digest, Sha256};

pub fn hex_digest(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::hex_digest;

    #[test]
    fn matches_published_vectors_across_padding_boundaries() {
        let cases: [(&[u8], &str); 4] = [
            (b"", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"),
            (b"abc", "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"),
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
            (
                b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
                "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(hex_digest(input), expected);
        }
        assert_eq!(
            hex_digest(&vec![b'a'; 1_000_000]),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
        let zeros = [
            (
                55,
                "02779466cdec163811d078815c633f21901413081449002f24aa3e80f0b88ef7",
            ),
            (
                56,
                "d4817aa5497628e7c77e6b606107042bbba3130888c5f47a375e6179be789fbb",
            ),
            (
                63,
                "c7723fa1e0127975e49e62e753db53924c1bd84b8ac1ac08df78d09270f3d971",
            ),
            (
                64,
                "f5a5fd42d16a20302798ef6ed309979b43003d2320d9f0e8ea9831a92759fb4b",
            ),
            (
                119,
                "f616b0d54e78571a9611f343c9f8e022e859e920381ab0e4d3da01e193a7bd7e",
            ),
            (
                120,
                "6edd9f6f9cc92cded36e6c4a580933f9c9f1b90562b46903b806f21902a1a54f",
            ),
        ];
        for (length, expected) in zeros {
            assert_eq!(
                hex_digest(&vec![0u8; length]),
                expected,
                "{length} zero bytes"
            );
        }
    }
}
