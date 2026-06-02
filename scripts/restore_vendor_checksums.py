#!/usr/bin/env python3
"""重建 os/vendor/*/.cargo-checksum.json（OSCOMP 评测机会过滤隐藏文件，删掉它）。

cargo 的 vendored directory 源要求每个 crate 目录下有 .cargo-checksum.json。
评测机 clone 时按"隐藏文件"递归删除了它，导致离线构建在加载依赖阶段失败。
本脚本从 Cargo.lock（非隐藏，过滤后存活）离线重建这些文件。

用法:
  restore_vendor_checksums.py [OS_DIR]            # 重建（缺失才写）
  restore_vendor_checksums.py --verify [OS_DIR]   # 仅校验：比对计算值与现有文件
默认 OS_DIR = <脚本目录>/../os
"""
import json
import os
import re
import sys


def parse_lock(path):
    """Cargo.lock -> {(name, version): checksum}（仅含 registry crate）。"""
    cks, name, ver, ck = {}, None, None, None
    with open(path, encoding="utf-8") as f:
        for line in f:
            s = line.strip()
            if s == "[[package]]":
                if name and ver and ck:
                    cks[(name, ver)] = ck
                name = ver = ck = None
            elif s.startswith("name ="):
                name = s.split("=", 1)[1].strip().strip('"')
            elif s.startswith("version ="):
                ver = s.split("=", 1)[1].strip().strip('"')
            elif s.startswith("checksum ="):
                ck = s.split("=", 1)[1].strip().strip('"')
    if name and ver and ck:
        cks[(name, ver)] = ck
    return cks


def pkg_name_ver(cargo_toml):
    """从 vendored Cargo.toml 的 [package] 表读 name/version（已规范化为具体值）。"""
    name = ver = None
    in_pkg = False
    with open(cargo_toml, encoding="utf-8") as f:
        for line in f:
            s = line.strip()
            if s.startswith("["):
                in_pkg = s == "[package]"
                continue
            if in_pkg:
                if name is None:
                    m = re.match(r'name\s*=\s*"([^"]+)"', s)
                    if m:
                        name = m.group(1)
                if ver is None:
                    m = re.match(r'version\s*=\s*"([^"]+)"', s)
                    if m:
                        ver = m.group(1)
            if name and ver:
                break
    return name, ver


def main(argv):
    verify = "--verify" in argv
    argv = [a for a in argv if a != "--verify"]
    script_dir = os.path.dirname(os.path.abspath(__file__))
    os_dir = os.path.abspath(argv[1]) if len(argv) > 1 else os.path.normpath(
        os.path.join(script_dir, "..", "os"))
    lock = os.path.join(os_dir, "Cargo.lock")
    vendor = os.path.join(os_dir, "vendor")

    if not os.path.isdir(vendor):
        print(f"[checksum] 无 {vendor}，跳过")
        return 0
    cks = parse_lock(lock)

    made = kept = miss = mismatch = ok = 0
    for d in sorted(os.listdir(vendor)):
        cdir = os.path.join(vendor, d)
        ctoml = os.path.join(cdir, "Cargo.toml")
        if not os.path.isfile(ctoml):
            continue
        name, ver = pkg_name_ver(ctoml)
        ck = cks.get((name, ver))
        ckfile = os.path.join(cdir, ".cargo-checksum.json")

        if verify:
            if not os.path.isfile(ckfile):
                print(f"[checksum] (verify) 缺文件 {d}")
                miss += 1
                continue
            with open(ckfile) as f:
                cur = json.load(f).get("package")
            if cur == ck:
                ok += 1
            else:
                print(f"[checksum] (verify) 不匹配 {name} {ver}: lock={ck} file={cur}")
                mismatch += 1
            continue

        if os.path.exists(ckfile):
            kept += 1
            continue
        if ck is None:
            print(f"[checksum] 警告: Cargo.lock 无 {name} {ver}（目录 {d}）")
            data = {"files": {}}
            miss += 1
        else:
            data = {"files": {}, "package": ck}
        with open(ckfile, "w", encoding="utf-8") as f:
            json.dump(data, f)
        made += 1

    if verify:
        print(f"[checksum] verify: 匹配={ok} 不匹配={mismatch} 缺文件={miss}")
        return 1 if mismatch else 0
    print(f"[checksum] 重建={made} 保留={kept} 缺checksum={miss}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
