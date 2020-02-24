#ifndef LIBPOLLPOLL_H
#define LIBPOLLPOLL_H

#include "Subscriber.h"
#include "Error.h"
#include "Alarm.h"
#include <map>

class Subscriber;

class Alarm;

class Poll {
private:
	static constexpr int WAIT_QUANTUM = 100;
	std::vector<pollfd> fds;
	std::unordered_map<int, std::shared_ptr<Subscriber>> subs;
	std::map<uint64_t, std::shared_ptr<Alarm>> alarms;
	bool shutdown;

	void compact();

public:

	Poll &subscribe(std::shared_ptr<Subscriber> sub);

	Poll &subscribe_alarm(std::shared_ptr<Alarm> alarm);

	void unsubscribe(Subscriber &sub);

	void loop();

	void do_shutdown();

	void notify_subscriber_changed(Subscriber &s);

	Poll();
};

#endif //LIBPOLLPOLL_H
