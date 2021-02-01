########### BUILD TOOL HELPERS #################

function buildloop_run() {
	clear && cargo build && run_integration_test ./tests/12
}

function buildloop() {
    while true; do
    	date
    	nc -l 7777
      bash -c "source build.sh && buildloop_run"
    done
}

# example: build_appimage ./target/debug/kbct ~/Downloads/appimagetool-x86_64.AppImage
function build_appimage() {
	binary=$1 && \
	app_image_tool=$2 && \
	dir="/tmp/KbctAppDir" && \
	rm -rf "$dir" && \
	mkdir -p "$dir/usr/lib" "$dir/usr/bin" && \
	library=$(ldconfig -p | grep libudev.so.1 | awk '{ print $4 }') && \
	cp "$library" "$dir/usr/lib" && \
	cp "$binary" "$dir/usr/bin" && \
	touch "$dir/icon.svg" && \

	cat  << EOF > "$dir/AppRun"
#!/bin/sh

cd "\$(dirname "\$0")"

exec ./usr/bin/kbct "\$@"
EOF

	cat << EOF > "$dir/kbct.desktop"
[Desktop Entry]
Name=kbct
Icon=icon
Type=Application
Categories=Utility;
EOF

	curr=$(pwd) && \
	cd "$dir" && \
	chmod +x AppRun && \
	"$app_image_tool" "./" && \
	cd "$curr" && \
	echo "Generated appimage in $dir"
}

########### UTILS FOR TESTING ######################


function test_fail() {
	cat << EOF | bash
	echo
	echo
	echo "$(tput setaf 1)****************************************$(tput sgr0)"
	echo "$(tput setaf 1)************* Test failed **************$(tput sgr0)"
	echo "$(tput setaf 1)****************************************$(tput sgr0)"
	exit 1
EOF
}

function test_passed() {
	echo
	echo
	echo "$(tput setaf 2)****************************************$(tput sgr0)"
	echo "$(tput setaf 2)************* Test passed **************$(tput sgr0)"
	echo "$(tput setaf 2)****************************************$(tput sgr0)"
}


function run_integration_test(){
	sudo echo ""
	sleep 0.2
	dir=$1
	echo "Running tests in $dir"

	sudo echo ""

	export RUST_BACKTRACE=1

	sudo -E kbct -d remap -c "$dir/conf.yaml" &
	sudo_pid=$!

	sudo -S -E kbct -d test-replay -t "$dir/test.txt"
	test_status=$?

  kbct_pid=$(pgrep -f "^kbct.*test" | tail -n1)

  sudo kill "$kbct_pid"
	wait "$sudo_pid"

	unset RUST_LOG
	unset RUST_BACKTRACE

	[[ test_status -eq "0" ]] && \
	 echo "$(tput setaf 2)Passed test $dir$(tput sgr0)" || \
	 (echo "Error in test $dir" && test_fail)

}

function run_all_integration_tests() {
	for dir in ./tests/*; do
		echo "$dir"
		run_integration_test "$dir" || break
	done && \
  test_passed

}

function run_all_tests() {
	(cargo test || test_fail) && \
	run_all_integration_tests
}