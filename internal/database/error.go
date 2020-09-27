package database

import (
	errors1 "errors"
	"fmt"
	"regexp"
	"strings"

	"github.com/jackc/pgx/v4"

	"github.com/jackc/pgerrcode"

	"github.com/JacobSoderblom/krypin/internal/errors"

	"github.com/jackc/pgconn"
)

func ConvertError(errMap map[string]error) func(error) error {

	return func(err error) error {
		var pgErr *pgconn.PgError

		if !errors1.As(err, &pgErr) {
			out, found := errMap[err.Error()]
			if !found {
				return err
			}

			return out
		}

		out, found := errMap[pgErr.Code]
		if !found {
			return err
		}

		return fmt.Errorf("%w: %v", out, pgErr)
	}
}

var columnFinder = regexp.MustCompile(`Key \((.+)\)=`)
var valueFinder = regexp.MustCompile(`Key \(.+\)=\((.+)\)`)

func findColumn(detail string) string {
	results := columnFinder.FindStringSubmatch(detail)
	if len(results) < 2 {
		return ""
	} else {
		return results[1]
	}
}

func findValue(detail string) string {
	results := valueFinder.FindStringSubmatch(detail)
	if len(results) < 2 {
		return ""
	} else {
		return results[1]
	}
}

var foreignKeyFinder = regexp.MustCompile(`not present in table "(.+)"`)

func findForeignKeyTable(detail string) string {
	results := foreignKeyFinder.FindStringSubmatch(detail)
	if len(results) < 2 {
		return ""
	}
	return results[1]
}

var parentTableFinder = regexp.MustCompile(`update or delete on table "([^"]+)"`)

func findParentTable(message string) string {
	match := parentTableFinder.FindStringSubmatch(message)
	if len(match) < 2 {
		return ""
	}
	return match[1]
}

func GetError(err error) error {
	var pgErr *pgconn.PgError

	if !errors1.As(err, &pgErr) {
		if errors1.Is(err, pgx.ErrNoRows) {
			return errors.New(errors.NotFound, "No rows was found")
		}

		return errors.Error{Err: err}
	}

	switch pgErr.Code {
	case pgerrcode.UniqueViolation:
		column := findColumn(pgErr.Detail)
		if column == "" {
			column = "value"
		}

		value := findValue(pgErr.Detail)

		var msg string

		if value == "" {
			msg = fmt.Sprintf("A %s already exists with that value in %s", column, pgErr.TableName)
		} else {
			msg = fmt.Sprintf("A %s already exists with value (%s) in %s", column, value, pgErr.TableName)
		}

		e := errors.New(errors.Conflict, msg)

		return e

	case pgerrcode.ForeignKeyViolation:
		columnName := findColumn(pgErr.Detail)
		if columnName == "" {
			columnName = "value"
		}
		foreignKeyTable := findForeignKeyTable(pgErr.Detail)
		var tablePart string
		if foreignKeyTable == "" {
			tablePart = "in the parent table"
		} else {
			tablePart = fmt.Sprintf("in the %s table", foreignKeyTable)
		}
		valueName := findValue(pgErr.Detail)
		var msg string
		var code errors.Code

		switch {
		case strings.Contains(pgErr.Message, "update or delete"):
			parentTable := findParentTable(pgErr.Message)
			// in this case pqerr.Table contains the child table. there's
			// probably more work we could do here.
			msg = fmt.Sprintf("Can't update or delete %[1]s records because the %[1]s %s (%s) is still referenced by the %s table", parentTable, columnName, valueName, pgErr.TableName)
			code = errors.Internal
		case valueName == "":
			msg = fmt.Sprintf("Can't save to %s because the %s isn't present %s", pgErr.TableName, columnName, tablePart)
			code = errors.NotFound
		default:
			msg = fmt.Sprintf("Can't save to %s because the %s (%s) isn't present %s", pgErr.TableName, columnName, valueName, tablePart)
			code = errors.NotFound
		}
		return errors.New(code, msg)

	case pgerrcode.NumericValueOutOfRange:
		msg := strings.Replace(pgErr.Message, "out of range", "too large or too small", 1)
		return errors.New(errors.Invalid, msg)

	case pgerrcode.InvalidTextRepresentation:
		msg := pgErr.Message
		// Postgres tweaks with the message, play whack-a-mole until we
		// figure out a better method of dealing with these.
		if !strings.Contains(pgErr.Message, "invalid input syntax for type") {
			msg = strings.Replace(pgErr.Message, "input syntax for", "input syntax for type", 1)
		}
		msg = strings.Replace(msg, "input value for enum ", "", 1)
		msg = strings.Replace(msg, "invalid", "Invalid", 1)
		return errors.New(errors.Internal, msg)

	case pgerrcode.NotNullViolation:
		msg := fmt.Sprintf("No %[1]s was provided. Please provide a %[1]s", pgErr.ColumnName)
		return errors.New(errors.Invalid, msg)

	default:
		return errors.Error{Err: err}
	}
}

func IsNoRows(err error) bool {
	return errors1.Is(err, pgx.ErrNoRows) || err == pgx.ErrNoRows
}
