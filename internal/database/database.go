package database

import (
	"context"
	"fmt"
	"os"

	"github.com/jackc/pgtype"
	pgtypeuuid "github.com/jackc/pgtype/ext/gofrs-uuid"
	"github.com/jackc/pgx/v4"
	"github.com/jackc/pgx/v4/pgxpool"
)

type transactionKey struct{}

type Database struct {
	pool *pgxpool.Pool
}

func (d *Database) Tx(ctx context.Context) *pgxpool.Tx {
	tx, _ := ctx.Value(transactionKey{}).(*pgxpool.Tx)

	return tx
}

func (d *Database) Close() {
	d.pool.Close()
}

func (d *Database) BatchTx(ctx context.Context, batch *pgx.Batch, fn func(res pgx.BatchResults) error) (err error) {
	tx := d.Tx(ctx)

	res := tx.SendBatch(ctx, batch)

	defer func() {
		res.Close()
	}()

	return fn(res)
}

func Connect(ctx context.Context) (*Database, error) {
	cfg, err := pgxpool.ParseConfig(os.Getenv("DATABASE_URL"))
	if err != nil {
		return nil, err
	}

	cfg.AfterConnect = func(ctx context.Context, conn *pgx.Conn) error {
		conn.ConnInfo().RegisterDataType(pgtype.DataType{
			Value: &pgtypeuuid.UUID{},
			Name:  "uuid",
			OID:   pgtype.UUIDOID,
		})
		return nil
	}

	conn, err := pgxpool.Connect(ctx, cfg.ConnString())
	if err != nil {
		return nil, fmt.Errorf("could not connect to database: %w", err)
	}

	return &Database{
		pool: conn,
	}, nil
}

func WithTransactionContext(ctx context.Context, db *Database, fn func(context.Context) error) (err error) {
	tx, err := db.pool.Begin(ctx)
	if err != nil {
		return fmt.Errorf("could not begin transaction from pool: %w", err)
	}

	defer func() {
		if p := recover(); p != nil {
			tx.Rollback(ctx)
			panic(p)
		}

		if err != nil {
			tx.Rollback(ctx)

			return
		}

		err = tx.Commit(ctx)
	}()

	return fn(context.WithValue(ctx, transactionKey{}, tx))
}
