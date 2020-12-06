
function buildloop_run() {
	clear && cargo build -v
}

function buildloop() {
    while true; do
    	date
    	nc -l 7777
      bash -c "source build.sh && buildloop_run"
    done
}

function integration_test() {

	for dir in ./tests/*; do
		echo "Running tests in $dir"
		sudo test-util replay --testcase "$dir/test.txt" --config "$dir/conf.yaml"
		if [[ $? -eq 0 ]]; then
			echo -e "\e[32mPassed\e[39m\n"
		else
			break
		fi
	done

}