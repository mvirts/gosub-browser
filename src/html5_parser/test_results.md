The following tests will fail, for the given reasons:



`<!DOCTYPE a PUBLIC'\uDBC0\uDC00`

This test has a non-bmp character that is internally seen as a single character but from the perspective 
of the test seen as 2 characters (hi/lo surrogate). This means that the end-of-file is off by 1 position.