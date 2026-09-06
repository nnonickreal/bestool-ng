# bestool-ng
experimental fork of the amazing [bestool by Ralim](https://github.com/Ralim/bestool) with some improvements :)

please note that this fork was written with LLM assistance, but has been fully tested on three different SoCs, use at your own risk.

## what's new?

i added new options and made the tool more resilient to unexpected chip states. 

for example, if the SoC is stuck in programmer mode while reading or writing due to a poor UART connection, tool should detect the chip's state immediately and will resume immediately from the stage the chip is currently in (because resetting the chip on audio devices is often not easy).

also, now it should work with all programmer blobs (and therefore chips as well), since it now dynamically reads information from the blobs and sends it to the chip.

**tested on:** `bes2300p`, `bes1600 (2700YH)` and `bes1502x (2700L)`

## BES SoCs support

i left all the programmer blobs from the [latest publicly available official BES programming tool](https://github.com/Derek-Vencer/tws_earbuds/tree/main/BES_DldProductLine_VS1.70.8) in the `programmers/` directory.

the rights to these blobs belong to BES (as do the rights to the default `programmer.bin` file in the project's root directory).

## usage & installation

### clone the repository locally

```bash
git clone --recursive https://github.com/nnonickreal/bestool-ng
```

### build

use [Rustup](https://rustup.rs/) if you don't have a rust toolchain installed.

```bash
cd bestool-ng/bestool
cargo build --release
```

### run & options

this fork adds new options for the `write-image` and `read-image` commands so that the tool can be used with all BES chips and is generally easier to use.

| option | feature | example |
| :--- | :--- | :--- |
| `--programmer` / `-P` | specify a custom programmer blob | `-P programmers/programmer1502x.bin` |
| `--start-address` / `-a` | specify the custom flash base address | `-a 0x24000000` |
| `--length` / `-l` (stock) | specify the custom reading length (works only with `read-image`) | `-l 0x18000` |
| `--offset` / `-o` (stock) | specify a custom offset relative to the base address | `-o 0x18000` |

for example:
```bash
./target/release/bestool read-image \
  --port /dev/ttyACM0 \
  -a 0x2c000000 \
  -o 0x20000 \
  -l 0x800000 \
  -P programmers/programmer1600.bin \
  my_amazing_firmware.bin
```

run with `bestool <subcommand> --help` to view available options for the commands.
