#!/bin/bash

if sudo cp ~/Documenten/GitHub/swyh-rs/target/release/swyh-rs /usr/bin/swyh-rs 
then
	[[ -d AppDir ]] && rm -r AppDir

	./linuxdeploy-x86_64.AppImage -e /usr/bin/swyh-rs -d ~/Documenten/GitHub/swyh-rs/deploy/swyh-rs.desktop -i ~/Documenten/GitHub/swyh-rs/deploy/n256.png --appdir AppDir --output appimage

	sudo rm /usr/bin/swyh-rs
fi

if sudo cp ~/Documenten/GitHub/swyh-rs/target/release/swyh-rs-cli /usr/bin/swyh-rs-cli 
then
	[[ -d AppDir ]] && rm -r AppDir

	./linuxdeploy-x86_64.AppImage -e /usr/bin/swyh-rs-cli -d ~/Documenten/GitHub/swyh-rs/deploy/swyh-rs-cli.desktop -i ~/Documenten/GitHub/swyh-rs/deploy/n256.png --appdir AppDir --output appimage

	sudo rm /usr/bin/swyh-rs-cli
fi


