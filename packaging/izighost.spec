Name:           izighost
Version:        0.1.0
Release:        1%{?dist}
Summary:        Десктопный AI-ассистент для технических собеседований

License:        WTFPL
URL:            https://github.com/izighost/izighost
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  gcc
BuildRequires:  clang-devel
BuildRequires:  pkgconfig
BuildRequires:  pkgconfig(fontconfig)
BuildRequires:  pkgconfig(libsecret-1)
BuildRequires:  pkgconfig(xkbcommon)
BuildRequires:  pkgconfig(wayland-client)
BuildRequires:  leptonica-devel
BuildRequires:  tesseract-devel

Requires:       dbus
Requires:       gnome-shell
Requires:       gnome-extensions-app
Requires:       python3
Requires:       python3-gobject
Requires:       zip
Requires:       gstreamer1
Requires:       gstreamer1-plugins-base
Requires:       gstreamer1-plugins-good
Requires:       tesseract

%description
IziGhost состоит из GUI-приложения и фонового D-Bus-демона. Оба компонента,
а также файлы D-Bus-активации и пользовательского systemd-сервиса,
поставляются одним RPM-пакетом.

%prep
%autosetup

%build
cargo build --release --locked

%install
install -Dm0755 target/release/izighost \
    %{buildroot}%{_bindir}/izighost
install -Dm0755 target/release/izighost-daemon \
    %{buildroot}%{_bindir}/izighost-daemon
install -Dm0644 installer/dbus/com.izighost.Daemon.service \
    %{buildroot}%{_datadir}/dbus-1/services/com.izighost.Daemon.service
install -Dm0644 installer/systemd/izighost-daemon.service \
    %{buildroot}%{_prefix}/lib/systemd/user/izighost-daemon.service

%files
%license LICENSE
%{_bindir}/izighost
%{_bindir}/izighost-daemon
%{_datadir}/dbus-1/services/com.izighost.Daemon.service
%{_prefix}/lib/systemd/user/izighost-daemon.service

%changelog
* Thu Jun 18 2026 IziGhost <dev@izighost.local> - 0.1.0-1
- Первый единый RPM-пакет приложения и демона
