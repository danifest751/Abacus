import random

from reference import int8


def test_honest_product_verifies_and_tampering_is_rejected():
    rng = random.Random(8)
    n = 8
    a = [rng.randrange(-128, 128) for _ in range(n * n)]
    b = [rng.randrange(-128, 128) for _ in range(n * n)]
    c = int8.matmul_i8(a, b, n)
    assert int8.verify_fs_i8(b"ph", a, b, c, n, 2)
    for delta in (1, -1, 2**31 - 1 - c[3]):
        bad = list(c)
        bad[3] += delta
        if -(2**31) <= bad[3] < 2**31:
            assert not int8.verify_fs_i8(b"ph", a, b, bad, n, 2)


def test_extremes_fit_int32_and_errors_do_not_vanish_mod_p():
    n = int8.MAX_N
    assert n * 128 * 128 < 2**31
    assert n * 128 * 128 + 2**31 < int8.P  # any integer error is nonzero mod P


def test_instance_bytes_are_signed_and_deterministic():
    a, b = int8.instance_i8(b"ph", 4)
    assert len(a) == 16 and len(b) == 16 and all(-128 <= x <= 127 for x in a + b)
    assert (a, b) == int8.instance_i8(b"ph", 4)
    assert (a, b) != int8.instance_i8(b"ph2", 4)
