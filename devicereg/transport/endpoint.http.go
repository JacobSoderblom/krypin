package transport

import (
	"net/http"

	"github.com/JacobSoderblom/krypin/internal/errors"

	"github.com/JacobSoderblom/krypin/devicereg"
	"github.com/gofrs/uuid"
	"github.com/labstack/echo/v4"
)

type EndpointSet struct {
	GetDevice echo.HandlerFunc
}

func Endpoints(svc devicereg.Service) EndpointSet {
	return EndpointSet{
		GetDevice: GetDevice(svc),
	}
}

func GetDevice(svc devicereg.Service) echo.HandlerFunc {
	return func(c echo.Context) error {
		id := c.Param("id")

		deviceID, err := uuid.FromString(id)
		if err != nil {
			return c.NoContent(http.StatusBadRequest)
		}

		d, err := svc.Get(c.Request().Context(), deviceID)
		if err != nil {
			if e, ok := errors.As(err); ok {
				switch e.Code {
				case errors.NotFound:
					return c.JSON(404, map[string]interface{}{
						"data": nil,
					})
				}
			}

			return c.JSON(http.StatusInternalServerError, err)
		}

		return c.JSON(200, map[string]interface{}{
			"data": d,
		})
	}
}
