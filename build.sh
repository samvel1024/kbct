
function buildloop_run() {
	clear && cargo build
}

function run_test_in_dir() {
	sudo test-util replay --testcase "$1/test.txt" --config "$1/conf.yaml" || test_fail ;
	test_passed
}

function run_all_tests() {
	cargo test || test_fail;
	integration_test
}

function test_fail() {
	echo
	echo
	echo "$(tput setaf 1)****************************************$(tput sgr0)"
	echo "$(tput setaf 1)************* Test failed **************$(tput sgr0)"
	echo "$(tput setaf 1)****************************************$(tput sgr0)"
	exit 1
}

function test_passed() {
	echo
	echo
	echo "$(tput setaf 2)****************************************$(tput sgr0)"
	echo "$(tput setaf 2)************* Test passed **************$(tput sgr0)"
	echo "$(tput setaf 2)****************************************$(tput sgr0)"
}

function buildloop() {
    while true; do
    	date
    	nc -l 7777
      bash -c "source build.sh && buildloop_run"
    done
}

function do_integration_test() {
	for dir in ./tests/*; do
		echo "Running tests in $dir"
		sudo kbct test-replay -t "$dir/test.txt" -c "$dir/conf.yaml" || test_fail
	done
  test_passed

}

function integration_test() {
	bash -c "source ./build.sh && do_integration_test"
}