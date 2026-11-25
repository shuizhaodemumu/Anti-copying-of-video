Name:           photo60-test
Version:        0.4.1
Release:        1%{?dist}
Summary:        Compare 60 Hz display modes with a phone camera
License:        MIT AND Apache-2.0 AND Unicode-3.0 AND (Apache-2.0 WITH LLVM-exception)
Source0:        %{name}-%{version}.tar.gz
ExclusiveArch:  x86_64

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  pkgconfig(gtk4) >= 4.8
BuildRequires:  desktop-file-utils
BuildRequires:  python3

%global debug_package %{nil}
%global appid io.github.shuizhaodemumu.Photo60Test

%description
A Rust/GTK test viewer for comparing a static Chinese reading sample and
a captioned motion clip on a 60 Hz monitor. The viewer can compare an alternating-color
watermark with a less luminance-modulated version for phone video recordings.
The viewer includes its offline test assets.

%prep
%autosetup

%build
# Keep the local account and cache path out of Rust panic locations.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=${CARGO_HOME:-$HOME/.cargo}=/usr/src/cargo --remap-path-prefix=/tmp=/usr/src/rpm"
cargo build --release --locked --offline --target-dir "${CARGO_TARGET_DIR:-$PWD/target}"
python3 packaging/crate-notices.py

%install
install -Dm0755 "${CARGO_TARGET_DIR:-$PWD/target}/release/%{name}" \
    "%{buildroot}%{_bindir}/%{name}"
desktop-file-install --dir="%{buildroot}%{_datadir}/applications" packaging/%{appid}.desktop

%check
"%{buildroot}%{_bindir}/%{name}" --version
desktop-file-validate "%{buildroot}%{_datadir}/applications/%{appid}.desktop"

%files
%doc packaging/README.zh-CN.md
%license LICENSE crate-notices
%{_bindir}/%{name}
%{_datadir}/applications/%{appid}.desktop
