#!/usr/bin/env python3
"""
Generate seed corpus files for fuzz testing.
These seed files provide structured starting points for libfuzzer.
"""

import struct
import os
from pathlib import Path

# ---------------------------------------------------------------------------
# XDR / ScVal encoding helpers
# ---------------------------------------------------------------------------
# ScVal is the XDR union type used for Soroban contract function arguments.
# Discriminants follow stellar-xdr curr (protocol 21): U32=0, I32=1, U64=2,
# I64=3, U128=4, I128=5, Bool=6, Symbol=7, Bytes=8, String=9, Vec=10,
# Map=11, Address=12, Void=19. XDR integers are big-endian; bytes and
# strings are length-prefixed and padded to 4-byte alignment.


def _pad4(b):
    return b + b"\x00" * ((-len(b)) % 4)


def xdr_u32(v):
    return struct.pack(">I", v)


def scval_u32(v):
    return xdr_u32(0) + xdr_u32(v)


def scval_i32(v):
    return xdr_u32(1) + struct.pack(">i", v)


def scval_u64(v):
    return xdr_u32(2) + struct.pack(">Q", v)


def scval_i64(v):
    return xdr_u32(3) + struct.pack(">q", v)


def scval_i128(v):
    return xdr_u32(5) + v.to_bytes(16, "big", signed=True)


def scval_bool(b):
    return xdr_u32(6) + xdr_u32(1 if b else 0)


def scval_bytes(b):
    return xdr_u32(8) + xdr_u32(len(b)) + _pad4(b)


def scval_string(s):
    raw = s.encode()
    return xdr_u32(9) + xdr_u32(len(raw)) + _pad4(raw)


def scval_vec(items):
    return xdr_u32(10) + xdr_u32(len(items)) + b"".join(items)


def scval_void():
    return xdr_u32(19)


def scval_address_contract(seed):
    # ScAddress::Contract(Hash) -> disc 1 + 32-byte hash
    return xdr_u32(12) + xdr_u32(1) + seed


def scval_address_account(seed):
    # ScAddress::Account(AccountId::Ed25519) -> disc 0 + account disc 0 + 32 bytes
    return xdr_u32(12) + xdr_u32(0) + xdr_u32(0) + seed


def scvec(items):
    # A Vec<ScVal> is the XDR form of a full contract argument list.
    return scval_vec(items)


def create_corpus_dirs():
    """Create corpus directories if they don't exist"""
    corpus_dirs = [
        "corpus/fuzz_create_vault",
        "corpus/fuzz_deposit",
        "corpus/fuzz_withdraw",
        "corpus/fuzz_check_in",
        "corpus/fuzz_xdr_scval",
        "corpus/fuzz_contract_xdr",
    ]
    for d in corpus_dirs:
        Path(d).mkdir(parents=True, exist_ok=True)

def generate_create_vault_corpus():
    """Generate seed inputs for create_vault fuzz target"""
    corpus_dir = Path("corpus/fuzz_create_vault")
    
    seeds = [
        # Valid: different owner and beneficiary, reasonable interval
        {
            "name": "valid_basic",
            "owner": b'\x00' * 32,
            "beneficiary": b'\x01' * 32,
            "interval": struct.pack('<Q', 86400),  # 1 day
            "has_token": 0,
        },
        # Invalid: zero interval
        {
            "name": "invalid_zero_interval",
            "owner": b'\x00' * 32,
            "beneficiary": b'\x01' * 32,
            "interval": struct.pack('<Q', 0),
            "has_token": 0,
        },
        # Invalid: owner equals beneficiary
        {
            "name": "invalid_same_owner_beneficiary",
            "owner": b'\x00' * 32,
            "beneficiary": b'\x00' * 32,
            "interval": struct.pack('<Q', 86400),
            "has_token": 0,
        },
        # Edge case: very long interval
        {
            "name": "edge_max_interval",
            "owner": b'\x00' * 32,
            "beneficiary": b'\x01' * 32,
            "interval": struct.pack('<Q', 0xFFFFFFFFFFFFFFFF),  # u64::MAX
            "has_token": 0,
        },
        # Valid: with custom token
        {
            "name": "valid_with_token",
            "owner": b'\x00' * 32,
            "beneficiary": b'\x01' * 32,
            "interval": struct.pack('<Q', 604800),  # 1 week
            "has_token": 1,
            "token": b'\x02' * 32,
        },
        # Boundary: minimum valid interval (1 second)
        {
            "name": "boundary_min_interval",
            "owner": b'\x00' * 32,
            "beneficiary": b'\x01' * 32,
            "interval": struct.pack('<Q', 1),
            "has_token": 0,
        },
    ]
    
    for seed in seeds:
        data = (
            seed["owner"] +
            seed["beneficiary"] +
            seed["interval"] +
            struct.pack('B', seed["has_token"])
        )
        if seed.get("token"):
            data += seed["token"]
        
        filepath = corpus_dir / seed["name"]
        filepath.write_bytes(data)
    
    print(f"✓ Created {len(seeds)} create_vault corpus seeds")

def generate_deposit_corpus():
    """Generate seed inputs for deposit fuzz target"""
    corpus_dir = Path("corpus/fuzz_deposit")
    
    seeds = [
        # Valid: simple deposit
        {
            "name": "valid_basic",
            "vault_id": struct.pack('<Q', 1),
            "depositor": b'\x00' * 32,
            "amount": struct.pack('<q', 1000),
            "current_balance": struct.pack('<q', 0),
            "max_deposit": struct.pack('<q', -1),  # i128 max (positive)
        },
        # Invalid: zero amount
        {
            "name": "invalid_zero_amount",
            "vault_id": struct.pack('<Q', 1),
            "depositor": b'\x00' * 32,
            "amount": struct.pack('<q', 0),
            "current_balance": struct.pack('<q', 0),
            "max_deposit": struct.pack('<q', -1),
        },
        # Invalid: negative amount
        {
            "name": "invalid_negative_amount",
            "vault_id": struct.pack('<Q', 1),
            "depositor": b'\x00' * 32,
            "amount": struct.pack('<q', -1000),
            "current_balance": struct.pack('<q', 0),
            "max_deposit": struct.pack('<q', -1),
        },
        # Edge case: balance overflow (approaching i128::MAX)
        {
            "name": "edge_balance_overflow",
            "vault_id": struct.pack('<Q', 1),
            "depositor": b'\x00' * 32,
            "amount": struct.pack('<q', 9223372036854775807),  # i64::MAX
            "current_balance": struct.pack('<q', 9223372036854775806),
            "max_deposit": struct.pack('<q', -1),
        },
        # Edge case: deposit limit exceeded
        {
            "name": "edge_deposit_limit_exceeded",
            "vault_id": struct.pack('<Q', 1),
            "depositor": b'\x00' * 32,
            "amount": struct.pack('<q', 1000),
            "current_balance": struct.pack('<q', 5000),
            "max_deposit": struct.pack('<q', 5000),
        },
        # Large deposit
        {
            "name": "large_deposit",
            "vault_id": struct.pack('<Q', 999),
            "depositor": b'\xFF' * 32,
            "amount": struct.pack('<q', 1000000000),
            "current_balance": struct.pack('<q', 1000000000),
            "max_deposit": struct.pack('<q', -1),
        },
    ]
    
    for seed in seeds:
        data = (
            seed["vault_id"] +
            seed["depositor"] +
            seed["amount"] +
            seed["current_balance"] +
            seed["max_deposit"]
        )
        filepath = corpus_dir / seed["name"]
        filepath.write_bytes(data)
    
    print(f"✓ Created {len(seeds)} deposit corpus seeds")

def generate_withdraw_corpus():
    """Generate seed inputs for withdraw fuzz target"""
    corpus_dir = Path("corpus/fuzz_withdraw")
    
    seeds = [
        # Valid: simple withdrawal
        {
            "name": "valid_basic",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "amount": struct.pack('<q', 100),
            "vault_balance": struct.pack('<q', 1000),
            "withdrawal_threshold": struct.pack('<q', -1),
            "min_balance_guard": struct.pack('<q', 0),
        },
        # Invalid: zero amount
        {
            "name": "invalid_zero_amount",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "amount": struct.pack('<q', 0),
            "vault_balance": struct.pack('<q', 1000),
            "withdrawal_threshold": struct.pack('<q', -1),
            "min_balance_guard": struct.pack('<q', 0),
        },
        # Invalid: insufficient balance
        {
            "name": "invalid_insufficient_balance",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "amount": struct.pack('<q', 2000),
            "vault_balance": struct.pack('<q', 1000),
            "withdrawal_threshold": struct.pack('<q', -1),
            "min_balance_guard": struct.pack('<q', 0),
        },
        # Edge case: withdrawal approval threshold
        {
            "name": "edge_withdrawal_threshold_exceeded",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "amount": struct.pack('<q', 1000),
            "vault_balance": struct.pack('<q', 10000),
            "withdrawal_threshold": struct.pack('<q', 500),
            "min_balance_guard": struct.pack('<q', 0),
        },
        # Edge case: minimum balance guard
        {
            "name": "edge_minimum_balance_guard",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "amount": struct.pack('<q', 6000),
            "vault_balance": struct.pack('<q', 10000),
            "withdrawal_threshold": struct.pack('<q', -1),
            "min_balance_guard": struct.pack('<q', 5000),
        },
        # Boundary: exact withdrawal to minimum
        {
            "name": "boundary_exact_minimum_balance",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "amount": struct.pack('<q', 5000),
            "vault_balance": struct.pack('<q', 10000),
            "withdrawal_threshold": struct.pack('<q', -1),
            "min_balance_guard": struct.pack('<q', 5000),
        },
    ]
    
    for seed in seeds:
        data = (
            seed["vault_id"] +
            seed["caller"] +
            seed["amount"] +
            seed["vault_balance"] +
            seed["withdrawal_threshold"] +
            seed["min_balance_guard"]
        )
        filepath = corpus_dir / seed["name"]
        filepath.write_bytes(data)
    
    print(f"✓ Created {len(seeds)} withdraw corpus seeds")

def generate_check_in_corpus():
    """Generate seed inputs for check_in fuzz target"""
    corpus_dir = Path("corpus/fuzz_check_in")
    
    seeds = [
        # Valid: on-time check-in
        {
            "name": "valid_on_time",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "passkey_hash": b'\x00' * 32,
            "nonce": struct.pack('<Q', 0),
            "current_time": struct.pack('<Q', 1000),
            "last_check_in": struct.pack('<Q', 100),
            "interval": struct.pack('<Q', 86400),
            "balance": struct.pack('<q', 10000),
            "penalty_bps": struct.pack('<H', 0),
        },
        # Invalid: zero interval
        {
            "name": "invalid_zero_interval",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "passkey_hash": b'\x00' * 32,
            "nonce": struct.pack('<Q', 0),
            "current_time": struct.pack('<Q', 1000),
            "last_check_in": struct.pack('<Q', 100),
            "interval": struct.pack('<Q', 0),
            "balance": struct.pack('<q', 10000),
            "penalty_bps": struct.pack('<H', 0),
        },
        # Invalid: vault expired (current_time >= last_check_in + interval)
        {
            "name": "invalid_vault_expired",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "passkey_hash": b'\x00' * 32,
            "nonce": struct.pack('<Q', 0),
            "current_time": struct.pack('<Q', 100000),
            "last_check_in": struct.pack('<Q', 100),
            "interval": struct.pack('<Q', 1000),
            "balance": struct.pack('<q', 10000),
            "penalty_bps": struct.pack('<H', 0),
        },
        # Edge case: TTL cap exceeded
        {
            "name": "edge_ttl_cap_exceeded",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "passkey_hash": b'\x00' * 32,
            "nonce": struct.pack('<Q', 0),
            "current_time": struct.pack('<Q', 1000),
            "last_check_in": struct.pack('<Q', 100),
            "interval": struct.pack('<Q', 0xFFFFFFFFFFFFFFFF),  # Very large interval
            "balance": struct.pack('<q', 10000),
            "penalty_bps": struct.pack('<H', 0),
        },
        # Edge case: inactivity penalty calculation
        {
            "name": "edge_inactivity_penalty",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "passkey_hash": b'\x00' * 32,
            "nonce": struct.pack('<Q', 0),
            "current_time": struct.pack('<Q', 1000000),
            "last_check_in": struct.pack('<Q', 100),
            "interval": struct.pack('<Q', 86400),
            "balance": struct.pack('<q', 10000),
            "penalty_bps": struct.pack('<H', 500),  # 5% penalty
        },
        # Boundary: check-in just before expiry
        {
            "name": "boundary_just_before_expiry",
            "vault_id": struct.pack('<Q', 1),
            "caller": b'\x00' * 32,
            "passkey_hash": b'\x00' * 32,
            "nonce": struct.pack('<Q', 0),
            "current_time": struct.pack('<Q', 86499),  # Just before expiry
            "last_check_in": struct.pack('<Q', 100),
            "interval": struct.pack('<Q', 86400),
            "balance": struct.pack('<q', 10000),
            "penalty_bps": struct.pack('<H', 0),
        },
    ]
    
    for seed in seeds:
        data = (
            seed["vault_id"] +
            seed["caller"] +
            seed["passkey_hash"] +
            seed["nonce"] +
            seed["current_time"] +
            seed["last_check_in"] +
            seed["interval"] +
            seed["balance"] +
            seed["penalty_bps"]
        )
        filepath = corpus_dir / seed["name"]
        filepath.write_bytes(data)
    
    print(f"✓ Created {len(seeds)} check_in corpus seeds")

def generate_xdr_scval_corpus():
    """Generate seed inputs for the raw XDR deserialization fuzz target.

    Seeds are raw XDR-encoded ScVals / ScVecs (the exact byte format the
    host deserializes for contract function arguments), plus malformed
    variants that must be rejected gracefully.
    """
    corpus_dir = Path("corpus/fuzz_xdr_scval")

    seeds = {
        # Valid single ScVals
        "valid_u32_zero": scval_u32(0),
        "valid_u64_one": scval_u64(1),
        "valid_u64_max": scval_u64(0xFFFFFFFFFFFFFFFF),
        "valid_i64_neg": scval_i64(-1),
        "valid_i128_min": scval_i128(-(2**127)),
        "valid_i128_max": scval_i128(2**127 - 1),
        "valid_bool_true": scval_bool(True),
        "valid_bool_false": scval_bool(False),
        "valid_void": scval_void(),
        "valid_empty_bytes": scval_bytes(b""),
        "valid_short_bytes": scval_bytes(b"\x01\x02\x03"),
        "valid_16_byte_payload": scval_bytes(bytes(range(16))),
        "valid_empty_string": scval_string(""),
        "valid_string": scval_string("reason"),
        "valid_empty_vec": scval_vec([]),
        "valid_nested_vec": scval_vec([scval_u64(1), scval_bool(False)]),
        "valid_address_contract": scval_address_contract(b"\x01" * 32),
        "valid_address_account": scval_address_account(b"\x02" * 32),
        # Valid ScVecs (full argument lists)
        "valid_scvec_two": scvec([scval_u64(1), scval_string("x")]),
        "valid_scvec_three": scvec(
            [scval_u64(1), scval_address_account(b"\x03" * 32), scval_i128(1000)]
        ),
        "valid_scvec_four": scvec(
            [
                scval_u64(1),
                scval_address_account(b"\x04" * 32),
                scval_bytes(b"\x05" * 32),
                scval_u64(0),
            ]
        ),
        # Malformed XDR: must decode to an error, never panic
        "malformed_unknown_discriminant": xdr_u32(99),
        "malformed_truncated_discriminant": b"\x00\x00",
        "malformed_truncated_bytes_len": xdr_u32(8) + xdr_u32(5) + b"\x01\x02",
        "malformed_truncated_vec_len": xdr_u32(10) + xdr_u32(3) + scval_u64(1),
        "malformed_bytes_oversize_len": xdr_u32(8) + struct.pack(">I", 0x7FFFFFFF),
        "malformed_empty": b"",
        "malformed_garbage": bytes(range(64)),
    }
    for name, data in seeds.items():
        (corpus_dir / name).write_bytes(data)
    print(f"✓ Created {len(seeds)} xdr_scval corpus seeds")


def generate_contract_xdr_corpus():
    """Generate seed inputs for the contract invocation fuzz target.

    Format: one mode byte (0 = single ScVal, 1 = ScVec, 2 = structured
    multi-sig payload) followed by the mode-specific bytes.
    """
    corpus_dir = Path("corpus/fuzz_contract_xdr")

    seeds = {
        # mode 0: single ScVal argument
        "single_pause_bytes": b"\x00" + scval_bytes(b"\xAA" * 16),
        "single_pause_empty_bytes": b"\x00" + scval_bytes(b""),
        "single_upgrade_bytes": b"\x00" + scval_bytes(b"\xBB" * 32),
        "single_whitelist_address": b"\x00" + scval_address_account(b"\x0C" * 32),
        "single_u64_mismatch": b"\x00" + scval_u64(7),
        "single_malformed": b"\x00" + xdr_u32(99),
        # mode 1: ScVec argument list
        "multi_file_dispute": b"\x01" + scvec([scval_u64(1), scval_string("dispute")]),
        "multi_deposit": b"\x01" + scvec(
            [scval_u64(1), scval_address_account(b"\x0D" * 32), scval_i128(1000)]
        ),
        "multi_check_in": b"\x01" + scvec(
            [
                scval_u64(1),
                scval_address_account(b"\x0E" * 32),
                scval_bytes(b"\x0F" * 32),
                scval_u64(0),
            ]
        ),
        # Unit-variant contracttype enums encode as Vec([Symbol(name)]);
        # Option::None encodes as an empty Vec.
        "multi_propose_multisig": b"\x01" + scvec(
            [
                scval_u64(1),
                scval_address_account(b"\x10" * 32),
                scval_vec([scval_string("Withdraw")]),
                scval_bytes(b"\x11" * 16),
                scval_vec([]),
            ]
        ),
        "multi_malformed": b"\x01" + b"\x00\x00",
        # mode 2: structured multi-sig payload
        # layout: op(1) + payload_len u16 LE(2) + payload + addr_flag(1)
        "multisig_withdraw_valid": b"\x02" + bytes([0]) + struct.pack("<H", 16) + (b"\x01" * 16) + bytes([0]),
        "multisig_withdraw_short": b"\x02" + bytes([0]) + struct.pack("<H", 4) + (b"\x01" * 4) + bytes([0]),
        "multisig_withdraw_empty": b"\x02" + bytes([0]) + struct.pack("<H", 0) + bytes([0]),
        "multisig_interval_valid": b"\x02" + bytes([3]) + struct.pack("<H", 8) + (b"\x02" * 8) + bytes([0]),
        "multisig_interval_short": b"\x02" + bytes([3]) + struct.pack("<H", 3) + (b"\x02" * 3) + bytes([0]),
        "multisig_beneficiary": b"\x02" + bytes([1]) + struct.pack("<H", 0) + bytes([1]),
        "multisig_cancel": b"\x02" + bytes([2]) + struct.pack("<H", 0) + bytes([0]),
        "multisig_truncated": b"\x02" + bytes([0]) + struct.pack("<H", 64) + (b"\x03" * 10),
    }
    for name, data in seeds.items():
        (corpus_dir / name).write_bytes(data)
    print(f"✓ Created {len(seeds)} contract_xdr corpus seeds")


if __name__ == "__main__":
    create_corpus_dirs()
    generate_create_vault_corpus()
    generate_deposit_corpus()
    generate_withdraw_corpus()
    generate_check_in_corpus()
    generate_xdr_scval_corpus()
    generate_contract_xdr_corpus()
    print("\n✓ All corpus files generated successfully!")
