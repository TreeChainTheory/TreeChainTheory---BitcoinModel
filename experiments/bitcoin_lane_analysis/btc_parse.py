"""Minimal parser for raw Bitcoin blocks (legacy and SegWit transactions).

parse_block() returns the transactions of a block with the fields the lane analysis
needs, recomputes every txid, and checks that the transactions hash to the Merkle root
in the block header, so a block that parses without an exception was read completely
and correctly.
"""
import hashlib
import struct

NULL_TXID = "00" * 32


def dsha(data):
    return hashlib.sha256(hashlib.sha256(data).digest()).digest()


def read_varint(buf, pos):
    n = buf[pos]
    if n < 0xFD:
        return n, pos + 1
    if n == 0xFD:
        return struct.unpack_from("<H", buf, pos + 1)[0], pos + 3
    if n == 0xFE:
        return struct.unpack_from("<I", buf, pos + 1)[0], pos + 5
    return struct.unpack_from("<Q", buf, pos + 1)[0], pos + 9


def parse_block(raw):
    """Return (header, txs); each tx is (txid, inputs), each input [prev_txid, vout, script_sig, witness]."""
    header = raw[:80]
    ntx, pos = read_varint(raw, 80)
    txs = []
    for _ in range(ntx):
        version = raw[pos:pos + 4]
        pos += 4
        segwit = raw[pos] == 0 and raw[pos + 1] == 1
        if segwit:
            pos += 2
        body_start = pos
        nin, pos = read_varint(raw, pos)
        inputs = []
        for _ in range(nin):
            prev_txid = raw[pos:pos + 32][::-1].hex()
            vout = struct.unpack_from("<I", raw, pos + 32)[0]
            slen, pos = read_varint(raw, pos + 36)
            inputs.append([prev_txid, vout, raw[pos:pos + slen], []])
            pos += slen + 4  # script_sig, sequence
        nout, pos = read_varint(raw, pos)
        for _ in range(nout):
            plen, pos = read_varint(raw, pos + 8)
            pos += plen
        body_end = pos
        if segwit:
            for inp in inputs:
                nitems, pos = read_varint(raw, pos)
                for _ in range(nitems):
                    ilen, pos = read_varint(raw, pos)
                    inp[3].append(raw[pos:pos + ilen])
                    pos += ilen
        locktime = raw[pos:pos + 4]
        pos += 4
        txid = dsha(version + raw[body_start:body_end] + locktime)
        txs.append((txid[::-1].hex(), inputs))
    if pos != len(raw):
        raise ValueError(f"{len(raw) - pos} unparsed bytes at the end of the block")
    layer = [bytes.fromhex(t)[::-1] for t, _ in txs]
    while len(layer) > 1:
        if len(layer) % 2:
            layer.append(layer[-1])
        layer = [dsha(layer[i] + layer[i + 1]) for i in range(0, len(layer), 2)]
    if layer[0] != header[36:68]:
        raise ValueError("transactions do not hash to the Merkle root in the header")
    return header, txs


def script_pushes(script):
    """Data pushes of a push-only script, or None if the script contains other opcodes."""
    pushes, pos = [], 0
    while pos < len(script):
        op = script[pos]
        pos += 1
        if op == 0:
            n = 0
        elif op <= 0x4B:
            n = op
        elif op == 0x4C:
            n, pos = script[pos], pos + 1
        elif op == 0x4D:
            n, pos = struct.unpack_from("<H", script, pos)[0], pos + 2
        elif op == 0x4E:
            n, pos = struct.unpack_from("<I", script, pos)[0], pos + 4
        else:
            return None
        if pos + n > len(script):
            return None
        pushes.append(script[pos:pos + n])
        pos += n
    return pushes


def is_pubkey(b):
    return (len(b) == 33 and b[0] in (2, 3)) or (len(b) == 65 and b[0] == 4)


def classify_input(script_sig, witness):
    """Return (kind, owner_key) for a transaction input.

    owner_key is the public key that authorizes the spend when it appears in the spending
    transaction (P2PKH, P2WPKH, P2SH-P2WPKH); otherwise None.
    """
    pushes = script_pushes(script_sig)
    if not witness:
        if pushes and len(pushes) == 2 and is_pubkey(pushes[1]) and 9 <= len(pushes[0]) <= 73:
            return "p2pkh", pushes[1]
        if pushes and len(pushes) >= 3 and pushes[0] == b"" and pushes[-1][-1:] == b"\xae":
            return "p2sh_multisig", None
        return "other_legacy", None
    if script_sig == b"" and len(witness) == 2 and len(witness[1]) == 33 and witness[1][0] in (2, 3):
        return "p2wpkh", witness[1]
    if (pushes and len(pushes) == 1 and len(pushes[0]) == 22 and pushes[0][:2] == b"\x00\x14"
            and len(witness) == 2 and len(witness[1]) == 33 and witness[1][0] in (2, 3)):
        return "p2sh_p2wpkh", witness[1]
    if script_sig == b"" and len(witness) == 1 and len(witness[0]) in (64, 65):
        return "p2tr_keypath", None
    if witness[-1][-1:] == b"\xae":
        return "p2wsh_multisig", None
    return "other_segwit", None


def hash160(data):
    return hashlib.new("ripemd160", hashlib.sha256(data).digest()).digest()


def _is_control_block(item):
    # taproot script-path spend: the last witness item is a control block (33 + 32k bytes, leaf version 0xc0/0xc1)
    return len(item) >= 33 and (len(item) - 33) % 32 == 0 and item[0] & 0xFE == 0xC0


def locking_script(script_sig, witness):
    """Rebuild the locking script (scriptPubKey) of the output an input spends, from what the
    spending input reveals. Returns (kind, script bytes), or (kind, None) when the spend does not
    reveal it (taproot, bare public keys, non-standard scripts)."""
    kind, key = classify_input(script_sig, witness)
    if kind == "p2pkh":
        return kind, b"\x76\xa9\x14" + hash160(key) + b"\x88\xac"
    if kind == "p2wpkh":
        return kind, b"\x00\x14" + hash160(key)
    if kind == "p2sh_p2wpkh":
        return kind, b"\xa9\x14" + hash160(b"\x00\x14" + hash160(key)) + b"\x87"
    pushes = script_pushes(script_sig)
    if kind == "p2sh_multisig":
        return kind, b"\xa9\x14" + hash160(pushes[-1]) + b"\x87"
    if witness:
        items = witness[:-1] if len(witness) >= 2 and witness[-1][:1] == b"\x50" else witness  # drop annex
        if pushes and len(pushes) == 1 and len(pushes[0]) == 34 and pushes[0][:2] == b"\x00\x20" and len(items) >= 2:
            return "p2sh_p2wsh", b"\xa9\x14" + hash160(pushes[0]) + b"\x87"
        if script_sig == b"" and len(items) >= 2 and not _is_control_block(items[-1]):
            return "p2wsh", b"\x00\x20" + hashlib.sha256(items[-1]).digest()
    return kind, None
