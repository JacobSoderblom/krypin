package database

import (
	"encoding/json"

	"github.com/jackc/pgtype"
)

func NewVarcharArray(l int) VarcharArray {
	return VarcharArray{
		VarcharArray: pgtype.VarcharArray{
			Status: pgtype.Null,
			Dimensions: []pgtype.ArrayDimension{
				pgtype.ArrayDimension{
					Length:     int32(l),
					LowerBound: 0,
				},
			},
		},
	}
}

type VarcharArray struct {
	pgtype.VarcharArray
}

func (a *VarcharArray) UnmarshalJSON(data []byte) error {
	var arr []string

	if err := json.Unmarshal(data, &arr); err != nil {
		return err
	}

	if arr == nil {
		a = &VarcharArray{
			VarcharArray: pgtype.VarcharArray{
				Status: pgtype.Null,
				Dimensions: []pgtype.ArrayDimension{
					pgtype.ArrayDimension{
						Length:     0,
						LowerBound: 0,
					},
				},
			},
		}

		return nil
	}

	a = &VarcharArray{
		VarcharArray: pgtype.VarcharArray{
			Status: pgtype.Present,
			Dimensions: []pgtype.ArrayDimension{
				pgtype.ArrayDimension{
					Length:     int32(len(arr)),
					LowerBound: 0,
				},
			},
		},
	}

	for _, s := range arr {
		a.Elements = append(a.Elements, pgtype.Varchar{String: s, Status: pgtype.Present})
	}

	return nil
}
