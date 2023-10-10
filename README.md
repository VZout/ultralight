<h1 align="center">
  Ultralight <img src="http://upload.vzout.com/ferris.svg" width="44px">
  <br>
  <a href="#"><img alt="GitHub Workflow Status (with event)" src="https://img.shields.io/github/actions/workflow/status/VZout/ultralight/rust.yml?style=flat-square&logo=github&logoColor=white"></a>
  <a href="https://crates.io/crates/ultralight"><img alt="Crates.io" src="https://img.shields.io/crates/v/ultralight?style=flat-square&logo=rust"></a>
  <a href="https://docs.rs/crate/ultralight/latest"><img alt="docs.rs" src="https://img.shields.io/docsrs/ultralight?style=flat-square"></a>
</p>

---

<p align="center">
  <strong>
  Opinionated <a href="https://www.rust-lang.org/">Rust</a> bindings for <a href="https://ultralig.ht/">Ultralight</a>
  </strong>
</p>

---


# Usage

The free version of Ultralight does not allow static linking. This crate however tries to automatically copy the DLL's for you. in the case of `WebCore.dll` it has to download it using the build script as its to large to upload to crates.io.

**Currently only windows is supported.**

# [Licence](https://ultralig.ht/#pricing)
