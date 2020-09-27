package timestamp_test

import (
	"testing"

	"github.com/JacobSoderblom/krypin/pkg/timestamp"
	"github.com/stretchr/testify/assert"
)

func TestParseTimestamp(t *testing.T) {
	assert := assert.New(t)

	ts := timestamp.Now()

	b, err := ts.MarshalJSON()
	assert.Nil(err)

	nTs := timestamp.Timestamp{}

	err = nTs.UnmarshalJSON(b)

	assert.Nil(err)

	assert.Equal(ts, nTs)
}
