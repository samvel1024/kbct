//
// Created by Samvel Abrahamyan on 2019-05-26.
//

#ifndef DISTFS_KILLRECEIVER_H
#define DISTFS_KILLRECEIVER_H
#ifdef __linux__


#include "Subscriber.h"

class KillReceiver : public Subscriber {
public:

	void on_input(Poll &p) override;

	void on_output(Poll &p) override;

	KillReceiver();
};

#else

/**
 * BSD and MacOS systems dont have signalfd syscall
 */
class KillReceiver : public Subscriber {
 public:
	virtual ~KillReceiver() {

	};

	KillReceiver() : Subscriber("Fake") {
		set_fd(-12345678);
		set_expected(0);
	}
};

#endif
#endif //DISTFS_KILLRECEIVER_H
