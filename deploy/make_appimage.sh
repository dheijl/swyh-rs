#!/bin/bash

SRC=~/Documenten/GitHub/swyh-rs

# pre-populate AppDir with the license files, linuxdeploy adds to an existing AppDir
prepare_appdir() {
	[[ -d AppDir ]] && rm -r AppDir
	mkdir -p AppDir/usr/share/doc/swyh-rs
	cp $SRC/LICENSE $SRC/THIRD-PARTY-LICENSES.md AppDir/usr/share/doc/swyh-rs/
}

if sudo cp $SRC/target/release/swyh-rs /usr/bin/swyh-rs
then
	prepare_appdir

	export LDAI_UPDATE_INFORMATION="gh-releases-zsync|dheijl|swyh-rs|latest|swyh-rs-x86_64.AppImage.zsync"

	./apps/linuxdeploy-x86_64.AppImage -e /usr/bin/swyh-rs -d $SRC/deploy/swyh-rs.desktop -i $SRC/deploy/n256.png --appdir AppDir --output appimage

	sudo rm /usr/bin/swyh-rs
fi

if sudo cp $SRC/target/release/swyh-rs-cli /usr/bin/swyh-rs-cli
then
	prepare_appdir

	export LDAI_UPDATE_INFORMATION="gh-releases-zsync|dheijl|swyh-rs|latest|swyh-rs-cli-x86_64.AppImage.zsync"

	./apps/linuxdeploy-x86_64.AppImage -e /usr/bin/swyh-rs-cli -d $SRC/deploy/swyh-rs-cli.desktop -i $SRC/deploy/n256.png --appdir AppDir --output appimage

	sudo rm /usr/bin/swyh-rs-cli
fi


