DefinitionBlock ("test.aml", "DSDT", 1, "OEMID", "TABLEID", 0x00000000)
{
    Scope (_SB)
    {
        Device (PCI0)
        {
            Name (_HID, EisaId ("PNP0A03"))
        }
    }
}
