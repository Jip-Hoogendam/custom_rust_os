// Name: Empty objects behave correctly
// Expect: int => 0

DefinitionBlock ("", "DSDT", 2, "uTEST", "TESTTABL", 0xF0F0F0F0)
{
    Name (FAIL, 0)

    Method (CHK, 3) {
        If (Arg1 != Arg2) {
            Printf ("%o: expected %o, got %o", Arg0,
                    ToHexString(Arg2), ToHexString(Arg1))
            FAIL++
        }
    }


    Method (EMPT) { }

    Method (MAIN) {
        Local0 = EMPT()

        CHK ("Type of an empty method", ObjectType (Local0), 0)

        Local1 = Package (0) { }
        CHK ("Type of an empty package", ObjectType (Local1), 4)
        CHK ("Size of an empty package", SizeOf (Local1), 0)

        Local4 = 0
        Local5 = Package(Local4) { }
        CHK ("Type of an empty package with a run-time size", ObjectType (Local5), 4)
        CHK ("Size of an empty package with a run-time size", SizeOf (Local5), 0)

        Return (FAIL)
    }
}
