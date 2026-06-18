NAME := izighost
VERSION := $(shell sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml)
RPM_TOPDIR := $(CURDIR)/build/rpm
SOURCE_ARCHIVE := $(RPM_TOPDIR)/SOURCES/$(NAME)-$(VERSION).tar.gz

.PHONY: rpm clean-rpm

rpm:
	@command -v rpmbuild >/dev/null || { echo "Ошибка: установите rpm-build"; exit 1; }
	mkdir -p $(RPM_TOPDIR)/BUILD $(RPM_TOPDIR)/BUILDROOT $(RPM_TOPDIR)/RPMS $(RPM_TOPDIR)/SOURCES $(RPM_TOPDIR)/SPECS $(RPM_TOPDIR)/SRPMS
	tar --exclude='./.git' --exclude='./target' --exclude='./build' \
		--transform='s,^\./,$(NAME)-$(VERSION)/,' \
		-czf $(SOURCE_ARCHIVE) .
	rpmbuild -bb \
		--define "_topdir $(RPM_TOPDIR)" \
		--define "_sourcedir $(RPM_TOPDIR)/SOURCES" \
		packaging/izighost.spec
	@find $(RPM_TOPDIR)/RPMS -type f -name '*.rpm' -print

clean-rpm:
	rm -rf $(RPM_TOPDIR)
