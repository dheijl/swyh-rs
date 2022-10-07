#!/bin/bash

if sudo cp ~/Documenten/GitHub/swyh-rs/target/release/swyh-rs /usr/bin/swyh-rs 
then
	[[ -f AppDir ]] && rm -r AppDir

	./linuxdeploy-x86_64.AppImage -e /usr/bin/swyh-rs -d ~/Documenten/GitHub/swyh-rs/deploy/swyh-rs.desktop -i ~/Documenten/GitHub/swyh-rs/deploy/n256.png --appdir AppDir --output appimage

	sudo rm /usr/bin/swyh-rs
fi



