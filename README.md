# Aion .pak 7-Zip Plugin

This repository contains a 7-Zip plugin for reading Aion `.pak` archives.

## Features

- **Aion CryPak support**: Open, browse, and extract Aion `.pak` archives using 7-Zip.
- **Automatic Binary XML decoding**: Detects binary-compressed XML files and automatically decodes them into UTF-8 XML
  data upon extraction.
- **Automatic HTML decryption**: Detects encrypted HTML files and automatically decrypts them upon extraction.

## Installing the 7-Zip Plugin

> [!NOTE]
> The DLL on the release page only works with the 64-bit version of 7-Zip for Windows. If you are using 7-Zip with a
different architecture or operating system, you will need to build the plugin yourself using
`cargo build -Z build-std=std --release` and copy the plugin from the `target/release` folder.

To enable Aion `.pak` support in 7-Zip, copy `aion_pak.dll` into the 7-Zip `Formats` directory (e.g.
`C:\Program Files\7-Zip\Formats`). The `Formats` folder must be created if it doesn't exist.

## Usage

Open any Aion `.pak` archive directly in 7-Zip File Manager or via CLI (`7z l data.pak`, `7z x data.pak -odata_out`).
