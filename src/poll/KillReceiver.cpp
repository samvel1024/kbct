//
// Created by Samvel Abrahamyan on 2019-05-26.
//
#ifdef __linux__

#include <signal.h>
#include "KillReceiver.h"
#include "Error.h"
#include <sys/signalfd.h>

void KillReceiver::on_input(Poll &p) {
	std::cout << this->name << ": Shutting down Poll" << std::endl;
	p.do_shutdown();
}

void KillReceiver::on_output(Poll &p) {

}

KillReceiver::KillReceiver() : Subscriber(std::string("KillReceiver")) {
	sigset_t mask;
	sigemptyset(&mask);
	sigaddset(&mask, SIGINT);
	no_err(sigprocmask(SIG_BLOCK, &mask, NULL), "sigprocmask failed");
	int fd = signalfd(-1, &mask, 0);
	set_fd(fd);
	set_expected(POLLIN | POLLERR | POLLHUP);
}

#endif