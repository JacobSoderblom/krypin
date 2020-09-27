package timestamp

import (
	"fmt"
	"strconv"
	"time"
)

type Timestamp struct {
	time.Time
}

func (t *Timestamp) MarshalJSON() ([]byte, error) {
	ts := t.Time.Unix()
	stamp := fmt.Sprint(ts)

	return []byte(stamp), nil
}

func (t *Timestamp) UnmarshalJSON(b []byte) error {
	ts, err := strconv.Atoi(string(b))
	if err != nil {
		return err
	}

	t.Time = time.Unix(int64(ts), 0).UTC()

	return nil
}

func Now() Timestamp {
	return Timestamp{
		Time: time.Unix(time.Now().Unix(), 0).UTC(),
	}
}

func Parse(timeformat, ts string) Timestamp {
	t, _ := time.Parse(timeformat, ts)
	return Timestamp{
		Time: t,
	}
}
