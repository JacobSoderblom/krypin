package http

import (
	"github.com/JacobSoderblom/krypin/devicereg/transport"
	"github.com/JacobSoderblom/krypin/internal/database"
	"github.com/labstack/echo/v4"
)

func NewHttpHandler(endpoints transport.EndpointSet, g *echo.Group, db *database.Database) *echo.Group {
	g = g.Group("/devices")

	g.Use(database.HttpTransaction(db))

	g.GET("/:id", endpoints.GetDevice)

	return g
}
