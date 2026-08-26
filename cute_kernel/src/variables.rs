// This module contains global variables to set the values in the kernal. This should reduce the
// usage of magic vairables.
// These should only contain variables that a res at compile time

///The amount of segment discriptors the GDT will hold
pub const GDT_TABLE_SIZE: usize = 7;

///The allocated size of the interupt stacks
pub type INTERUPT_STACK = [u8; 4096 * 64];
