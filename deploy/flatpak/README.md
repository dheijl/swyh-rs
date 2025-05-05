# swyh-rs flatpak experiment files

The files in this folder are the result of an experiment to build a flatpak version of swyh-rs.

While the flatpak build eventually succeeded thanks to [@yveszoundi](https://github.com/yveszoundi/fltk-rs-flatpak) helping me out with the static build of the entire GNU libstdc++ (needed by fltk-rs for including in a flatpak), I decided not to proceed as flatpaks don't work "out of the box" with Pipewire, at least not on Debian 12.

I leave the files here for reference.
