// Name: CopyObject on yourself works
// Expect: str => Hello World

DefinitionBlock ("", "DSDT", 2, "uTEST", "TESTTABL", 0xF0F0F0F0)
{
    Method (BAR, 1, Serialized) {
        CopyObject (Arg0, BAR)
    }

    Method (FOO) {
        CopyObject("Hello", FOO)
        BAR(" World")

        Return (0x123)
    }

    Method (MAIN) {
        Local0 = FOO()

        Return (Concatenate(FOO, BAR))
    }
}
